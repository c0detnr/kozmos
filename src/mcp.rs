//! On-demand MCP diagnostics. Never calls a server tool.
use rmcp::{
    ServiceExt,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

pub const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct Server {
    pub name: String,
    config: Value,
    pub enabled: bool,
    pub cwd: PathBuf,
}
impl Server {
    pub fn transport_label(&self) -> &'static str {
        if self.config.get("url").is_some() {
            "HTTP"
        } else {
            "stdio"
        }
    }
}

pub fn servers(path: &Path, raw: &str, cwd: &Path) -> Result<Vec<Server>, String> {
    let doc: Value = if path.extension().is_some_and(|e| e == "toml") {
        let value: toml::Value =
            toml::from_str(raw).map_err(|_| "TOML yapılandırması okunamadı.".to_string())?;
        serde_json::to_value(value).map_err(|_| "TOML dönüştürülemedi.".to_string())?
    } else {
        serde_json::from_str(raw).map_err(|_| "JSON yapılandırması okunamadı.".to_string())?
    };
    if !doc.is_object() {
        return Err("Yapılandırma bir nesne olmalı.".into());
    }
    let mut output = Vec::new();
    collect(&doc, cwd, "", &mut output)?;
    // Claude Code also stores project-scoped entries inside ~/.claude.json.
    if let Some(projects) = doc.get("projects").and_then(Value::as_object) {
        for (project, config) in projects {
            if Path::new(project) == cwd {
                collect(config, cwd, "Proje / ", &mut output)?;
            }
        }
    }
    output.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(output)
}
fn collect(doc: &Value, cwd: &Path, prefix: &str, out: &mut Vec<Server>) -> Result<(), String> {
    let Some(table) = doc
        .get("mcp_servers")
        .or_else(|| doc.get("mcpServers"))
        .or_else(|| doc.get("mcp"))
    else {
        return Ok(());
    };
    for (name, config) in table.as_object().ok_or("MCP alanı bir nesne olmalı.")? {
        out.push(Server {
            name: format!("{prefix}{name}"),
            enabled: config.get("enabled").and_then(Value::as_bool) != Some(false)
                && config.get("disabled").and_then(Value::as_bool) != Some(true),
            config: config.clone(),
            cwd: cwd.to_path_buf(),
        });
    }
    Ok(())
}

fn expand(raw: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut rest = raw;
    loop {
        let found = rest
            .find("${")
            .map(|i| (i, 2))
            .into_iter()
            .chain(rest.find("{env:").map(|i| (i, 5)))
            .min_by_key(|x| x.0);
        let Some((start, len)) = found else {
            result.push_str(rest);
            break;
        };
        result.push_str(&rest[..start]);
        let tail = &rest[start + len..];
        let end = tail
            .find('}')
            .ok_or("Ortam değişkeni ifadesi kapanmamış.")?;
        let (name, default) = tail[..end]
            .split_once(":-")
            .map_or((&tail[..end], None), |(k, v)| (k, Some(v)));
        let value = std::env::var(name)
            .ok()
            .or_else(|| default.map(str::to_string))
            .ok_or("Gerekli ortam değişkeni Kozmos oturumunda tanımlı değil.")?;
        result.push_str(&value);
        rest = &tail[end + 1..];
    }
    Ok(result)
}
fn string_map(config: &Value, key: &str) -> Result<HashMap<String, String>, String> {
    let Some(value) = config.get(key) else {
        return Ok(HashMap::new());
    };
    value
        .as_object()
        .ok_or_else(|| format!("{key} bir nesne olmalı."))?
        .iter()
        .map(|(k, v)| {
            Ok((
                k.clone(),
                expand(
                    v.as_str()
                        .ok_or_else(|| format!("{key} değerleri metin olmalı."))?,
                )?,
            ))
        })
        .collect()
}

pub struct Report {
    pub elapsed: Duration,
    pub outcome: Result<Details, String>,
}
pub struct Details {
    pub server: String,
    pub tools: Vec<(String, String)>,
    pub supports_tools: bool,
}

// Never expose server responses, command arguments, URLs or credentials in errors.
fn connection_error(stage: &str, error: impl std::fmt::Display) -> String {
    let text = error.to_string().to_lowercase();
    let detail = if text.contains("401") || text.contains("403") || text.contains("unauthorized") {
        "Yetkilendirme reddedildi; token veya HTTP başlıklarını kontrol edin."
    } else if text.contains("connection refused") || text.contains("connect error") {
        "Sunucuya ulaşılamadı; adresi ve sunucunun çalıştığını kontrol edin."
    } else if text.contains("404") || text.contains("405") {
        "MCP HTTP uç noktası bulunamadı veya Streamable HTTP desteklenmiyor."
    } else {
        "Sunucu kapandı, geçerli MCP yanıtı vermedi veya istek reddedildi."
    };
    format!("{stage}: {detail}")
}

struct Process(tokio::process::Child);
impl Drop for Process {
    fn drop(&mut self) {
        if let Some(pid) = self.0.id() {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
        let _ = self.0.start_kill();
    }
}

pub async fn probe(server: Server, mut cancel: oneshot::Receiver<()>, limit: Duration) -> Report {
    let started = Instant::now();
    let mut child = None;
    let outcome = tokio::select! {
        biased;
        _ = &mut cancel => Err("Test iptal edildi.".into()),
        _ = tokio::time::sleep(limit) => Err(format!("{} saniye içinde yanıt alınamadı; test durduruldu.", limit.as_secs())),
        result = connect(&server, &mut child) => result,
    };
    if let Some(mut process) = child {
        // Kill the process group as well, including launchers such as npx.
        if let Some(pid) = process.0.id() {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
        let _ = process.0.kill().await;
        let _ = process.0.wait().await;
    }
    Report {
        elapsed: started.elapsed(),
        outcome,
    }
}

async fn connect(server: &Server, child: &mut Option<Process>) -> Result<Details, String> {
    if !server.enabled {
        return Err("Sunucu yapılandırmada devre dışı.".into());
    }
    let c = &server.config;
    let kind = c.get("type").and_then(Value::as_str).unwrap_or("");
    if !["", "stdio", "local", "http", "streamable-http", "remote"].contains(&kind) {
        return Err("Bu taşıma türü desteklenmiyor. stdio veya Streamable HTTP kullanın; eski SSE ve OAuth oturum açma desteklenmiyor.".into());
    }
    let service = if let Some(url) = c.get("url") {
        let url = expand(url.as_str().ok_or("url bir metin olmalı.")?)?;
        let parsed = reqwest::Url::parse(&url).map_err(|_| "HTTP adresi geçersiz.")?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Yalnızca http/https adresleri desteklenir.".into());
        }
        let mut config = StreamableHttpClientTransportConfig::with_uri(url);
        let mut headers = string_map(c, "headers")?;
        headers.extend(string_map(c, "http_headers")?);
        for (name, var) in string_map(c, "env_http_headers")? {
            headers.insert(
                name,
                std::env::var(var).map_err(|_| "HTTP başlığı için ortam değişkeni eksik.")?,
            );
        }
        if let Some(var) = c.get("bearer_token_env_var") {
            config.auth_header = Some(
                std::env::var(var.as_str().ok_or("Token değişken adı metin olmalı.")?)
                    .map_err(|_| "Token ortam değişkeni tanımlı değil.")?,
            );
        }
        for (name, value) in headers {
            config.custom_headers.insert(
                name.parse().map_err(|_| "HTTP başlık adı geçersiz.")?,
                value.parse().map_err(|_| "HTTP başlık değeri geçersiz.")?,
            );
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(8))
            .build()
            .map_err(|_| "HTTP istemcisi oluşturulamadı.")?;
        ().serve(StreamableHttpClientTransport::with_client(client, config))
            .await
            .map_err(|e| connection_error("MCP el sıkışması", e))?
    } else {
        let command = c
            .get("command")
            .ok_or("Sunucu için command veya url eksik.")?;
        let (program, mut args) = if let Some(array) = command.as_array() {
            let mut parts = array
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or_else(|| "command dizisi metinlerden oluşmalı.".to_string())
                        .and_then(expand)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if parts.is_empty() {
                return Err("command boş olamaz.".into());
            }
            (parts.remove(0), parts)
        } else {
            (
                expand(command.as_str().ok_or("command metin veya dizi olmalı.")?)?,
                vec![],
            )
        };
        if let Some(values) = c.get("args") {
            for value in values.as_array().ok_or("args bir dizi olmalı.")? {
                args.push(expand(
                    value.as_str().ok_or("args değerleri metin olmalı.")?,
                )?);
            }
        }
        let mut command = tokio::process::Command::new(program);
        command
            .args(args)
            .envs(string_map(c, "env")?)
            .envs(string_map(c, "environment")?)
            .current_dir(&server.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .process_group(0);
        if let Some(cwd) = c.get("cwd") {
            let raw = expand(cwd.as_str().ok_or("cwd metin olmalı.")?)?;
            let cwd = server.cwd.join(raw);
            crate::workspace::check_path(&server.cwd, &cwd)?;
            command.current_dir(cwd);
        }
        let process = command.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                "Komut veya çalışma klasörü bulunamadı; PATH ve cwd alanlarını kontrol edin."
                    .to_string()
            }
            std::io::ErrorKind::PermissionDenied => "Sunucuyu başlatma izni yok.".to_string(),
            _ => "MCP sunucu süreci başlatılamadı.".to_string(),
        })?;
        let process = child.insert(Process(process));
        let stdout = process.0.stdout.take().ok_or("Sunucu çıktısı açılamadı.")?;
        let stdin = process.0.stdin.take().ok_or("Sunucu girdisi açılamadı.")?;
        ().serve((stdout, stdin))
            .await
            .map_err(|e| connection_error("MCP el sıkışması", e))?
    };
    let result = async {
        let info = service.peer_info().ok_or("Sunucu kimliği alınamadı.")?;
        let supports_tools = info.capabilities.tools.is_some();
        let tools = if supports_tools {
            service
                .list_all_tools()
                .await
                .map_err(|e| connection_error("Araç listesi", e))?
        } else {
            vec![]
        };
        Ok(Details {
            server: info
                .server_info
                .as_ref()
                .map(|i| format!("{} {}", i.name, i.version))
                .unwrap_or_else(|| "MCP sunucusu".into()),
            tools: tools
                .into_iter()
                .map(|t| {
                    (
                        t.name.to_string(),
                        t.description.map(|s| s.to_string()).unwrap_or_default(),
                    )
                })
                .collect(),
            supports_tools,
        })
    }
    .await;
    let _ = tokio::time::timeout(Duration::from_secs(1), service.cancel()).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    fn fixture(mode: &str) -> Server {
        Server {
            name: "fixture".into(),
            enabled: true,
            cwd: std::env::current_dir().unwrap(),
            config: json!({
                "command": ["python3", "-u", concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mcp_server.py"), mode],
                "environment": {"KOZMOS_FIXTURE": "environment-ok"}
            }),
        }
    }
    async fn test_probe(server: Server) -> Report {
        let (_cancel, rx) = oneshot::channel();
        probe(server, rx, Duration::from_secs(5)).await
    }
    #[test]
    fn parses_provider_formats_and_disabled_entries() {
        let cwd = Path::new("/tmp/project");
        let codex = servers(
            Path::new("config.toml"),
            "[mcp_servers.example]\ncommand = 'node'\nenabled = false",
            cwd,
        )
        .unwrap();
        assert_eq!(codex[0].name, "example");
        assert!(!codex[0].enabled);
        for raw in [
            r#"{"mcpServers":{"example":{"command":"node"}}}"#,
            r#"{"mcp":{"example":{"type":"local","command":["node"]}}}"#,
        ] {
            assert_eq!(
                servers(Path::new("mcp.json"), raw, cwd).unwrap()[0].cwd,
                cwd
            );
        }
        assert!(servers(Path::new("bad.json"), "{", cwd).is_err());
        assert!(servers(Path::new("bad.json"), "[]", cwd).is_err());
        assert!(servers(Path::new("bad.json"), r#"{"mcp":[]}"#, cwd).is_err());
    }
    #[test]
    fn claude_projects_only_include_selected_workspace() {
        let raw = r#"{"mcpServers":{"global":{}},"projects":{"/a":{"mcpServers":{"local":{}}},"/b":{"mcpServers":{"other":{}}}}}"#;
        let rows = servers(Path::new("claude.json"), raw, Path::new("/a")).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|s| s.name == "Proje / local"));
    }
    #[test]
    fn expands_environment_without_exposing_secrets() {
        let var = "KOZMOS_UNDEFINED_FIXTURE_984032";
        assert_eq!(
            expand(&format!("before${{{var}:-fallback}}after")).unwrap(),
            "beforefallbackafter"
        );
        assert!(expand(&format!("{{env:{var}}}")).is_err());
        assert_eq!(expand("${PATH}").unwrap(), std::env::var("PATH").unwrap());
        assert!(!connection_error("MCP", "401 private-secret").contains("private-secret"));
    }
    #[tokio::test]
    async fn stdio_handshake_pagination_and_environment() {
        let details = test_probe(fixture("normal")).await.outcome.unwrap();
        assert!(details.server.contains("kozmos-fixture"));
        assert_eq!(details.tools.len(), 2);
        assert_eq!(details.tools[1].0, "second");
        assert_eq!(details.tools[0].1, "environment-ok");
    }
    #[tokio::test]
    async fn stdio_working_directory_stays_inside_project() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("nested")).unwrap();
        std::os::unix::fs::symlink(dir.path(), root.join("outside")).unwrap();
        for cwd in ["..", "outside", "/tmp"] {
            let mut server = fixture("normal");
            server.cwd = root.clone();
            server.config["cwd"] = json!(cwd);
            let error = test_probe(server).await.outcome.err().unwrap();
            assert!(error.contains("çalışma dizininin dışında"), "{error}");
        }
        let mut server = fixture("normal");
        server.cwd = root;
        server.config["cwd"] = json!("nested");
        assert!(test_probe(server).await.outcome.is_ok());
    }
    #[tokio::test]
    async fn no_tools_capability_is_successful() {
        let details = test_probe(fixture("no-tools")).await.outcome.unwrap();
        assert!(!details.supports_tools);
        assert!(details.tools.is_empty());
    }
    #[tokio::test]
    async fn tool_listing_failure_is_reported_without_server_error_secrets() {
        let error = test_probe(fixture("reject")).await.outcome.err().unwrap();
        assert!(error.contains("Araç listesi"));
        assert!(!error.contains("private-token"));
    }
    #[tokio::test]
    async fn missing_command_disabled_and_unsupported_transport_fail() {
        let mut server = fixture("normal");
        server.config = json!({"command":"/nonexistent/kozmos-test-mcp"});
        assert!(
            test_probe(server.clone())
                .await
                .outcome
                .err()
                .unwrap()
                .contains("bulunamadı")
        );
        server.enabled = false;
        assert!(
            test_probe(server.clone())
                .await
                .outcome
                .err()
                .unwrap()
                .contains("devre dışı")
        );
        server.enabled = true;
        server.config = json!({"type":"sse","url":"http://127.0.0.1"});
        assert!(
            test_probe(server)
                .await
                .outcome
                .err()
                .unwrap()
                .contains("desteklenmiyor")
        );
    }
    async fn interrupted_test(cancelled: bool) {
        let dir = tempfile::tempdir().unwrap();
        let pids_file = dir.path().join("pids.json");
        let mut server = fixture("hang");
        server.config["command"]
            .as_array_mut()
            .unwrap()
            .push(json!(pids_file));
        let (cancel, rx) = oneshot::channel();
        let limit = if cancelled {
            Duration::from_secs(5)
        } else {
            Duration::from_millis(700)
        };
        let probe_task = tokio::spawn(probe(server, rx, limit));
        for _ in 0..100 {
            if pids_file.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        if cancelled {
            cancel.send(()).unwrap();
        }
        let report = probe_task.await.unwrap();
        let error = report.outcome.err().unwrap();
        assert!(error.contains(if cancelled {
            "iptal"
        } else {
            "yanıt alınamadı"
        }));
        let pids: Vec<u32> =
            serde_json::from_str(&std::fs::read_to_string(pids_file).unwrap()).unwrap();
        for pid in pids {
            // A killed process may be dead or a zombie before /proc removes it.
            if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
                assert!(matches!(stat.split_whitespace().nth(2), Some("Z" | "X")));
            }
        }
    }
    #[tokio::test]
    async fn timeout_kills_server_and_descendants() {
        interrupted_test(false).await;
    }
    #[tokio::test]
    async fn cancellation_kills_server_and_descendants() {
        interrupted_test(true).await;
    }

    struct HttpFixture {
        url: String,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl HttpFixture {
        fn new(status: u16) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/mcp", listener.local_addr().unwrap());
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let signal = stop.clone();
            let thread = std::thread::spawn(move || {
                while !signal.load(Ordering::Relaxed) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(s) => s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(e) => panic!("{e}"),
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut first = String::new();
                    reader.read_line(&mut first).unwrap();
                    let mut length = 0;
                    let mut auth = false;
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" || line.is_empty() {
                            break;
                        }
                        let lower = line.to_lowercase();
                        if let Some(value) = lower.strip_prefix("content-length:") {
                            length = value.trim().parse().unwrap();
                        }
                        if lower.trim() == "authorization: bearer fixture-secret" {
                            auth = true;
                        }
                    }
                    assert!(auth, "configured header must be sent");
                    let mut body = vec![0; length];
                    reader.read_exact(&mut body).unwrap();
                    let mut code = status;
                    let mut response = String::new();
                    if status == 200 && first.starts_with("POST") {
                        let request: Value = serde_json::from_slice(&body).unwrap();
                        let result = match request["method"].as_str().unwrap() {
                            "initialize" => {
                                json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"http-fixture","version":"1"}})
                            }
                            "tools/list" => json!({"tools":[]}),
                            "notifications/initialized" => {
                                code = 202;
                                Value::Null
                            }
                            other => panic!("unexpected method {other}"),
                        };
                        if code == 200 {
                            response = json!({"jsonrpc":"2.0","id":request["id"],"result":result})
                                .to_string();
                        }
                    } else if status == 200 {
                        code = 405;
                    }
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {code} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                        response.len()
                    );
                }
            });
            Self {
                url,
                stop,
                thread: Some(thread),
            }
        }
        fn server(&self) -> Server {
            Server {
                name: "http".into(),
                enabled: true,
                cwd: PathBuf::from("/tmp"),
                config: json!({"url":self.url,"headers":{"Authorization":"Bearer fixture-secret"}}),
            }
        }
    }
    impl Drop for HttpFixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
        }
    }
    #[tokio::test]
    async fn http_handshake_with_headers_and_empty_tools() {
        let fixture = HttpFixture::new(200);
        let details = test_probe(fixture.server()).await.outcome.unwrap();
        assert!(details.server.contains("http-fixture"));
        assert!(details.supports_tools && details.tools.is_empty());
    }
    #[tokio::test]
    async fn http_auth_failure_is_actionable_and_redacted() {
        let fixture = HttpFixture::new(401);
        let error = test_probe(fixture.server()).await.outcome.err().unwrap();
        assert!(error.contains("Yetkilendirme"), "{error}");
        assert!(!error.contains("fixture-secret"));
    }
}
