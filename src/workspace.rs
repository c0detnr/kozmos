use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    Skill,
    Mcp,
    Instructions,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Skill => "Skills",
            Self::Mcp => "MCP sunucuları",
            Self::Instructions => "Agent talimatları",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Resource {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub kind: Kind,
    pub global: bool,
    pub provider: String,
    pub enabled: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Source {
    pub path: PathBuf,
    pub kind: Kind,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub project_names: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub sources: Vec<Source>,
    pub projects: Vec<PathBuf>,
    pub active: Option<PathBuf>,
}
pub struct Workspace {
    pub home: PathBuf,
    pub settings: Settings,
    pub resources: Vec<Resource>,
    pub warnings: Vec<String>,
    state_path: PathBuf,
    state_original: Option<String>,
}
impl Workspace {
    pub fn new(home: PathBuf, state_path: PathBuf) -> Self {
        let mut warnings = Vec::new();
        let raw = match fs::read_to_string(&state_path) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                warnings.push(format!("Proje kaydı okunamadı: {e}"));
                None
            }
        };
        let settings = raw
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .unwrap_or_else(|e| {
                warnings.push(format!("Proje kaydı geçersiz: {e}"));
                Some(Settings::default())
            })
            .unwrap_or_default();
        let mut ws = Self {
            home,
            settings,
            resources: vec![],
            warnings,
            state_path,
            state_original: raw,
        };
        ws.refresh();
        ws
    }
    pub fn save_settings(&mut self) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
        save_file(&self.state_path, self.state_original.as_deref(), &raw, None)?;
        self.state_original = Some(raw);
        Ok(())
    }
    pub fn add_project(&mut self, path: &str) -> Result<(), String> {
        let path = expand_path(path, &self.home)
            .canonicalize()
            .map_err(|e| format!("Proje bulunamadı: {e}"))?;
        if !path.is_dir() {
            return Err("Bir proje klasörü seçin.".into());
        }
        let previous = self.settings.clone();
        self.settings.projects.retain(|project| project != &path);
        self.settings.projects.insert(0, path.clone());
        self.settings.active = Some(path);
        if let Err(error) = self.save_settings() {
            self.settings = previous;
            return Err(error);
        }
        self.refresh();
        Ok(())
    }
    pub fn add_source(&mut self, raw: &str, kind: Kind) -> Result<(), String> {
        let path = expand_path(raw, &self.home)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if kind == Kind::Mcp {
            if !path.is_file() {
                return Err("MCP için bir JSON veya TOML dosyası seçin.".into());
            }
            mcp_names(&path, &read_document(&path)?)?;
        }
        if !self
            .settings
            .sources
            .iter()
            .any(|s| s.path == path && s.kind == kind)
        {
            self.settings.sources.push(Source { path, kind });
        }
        self.save_settings()?;
        self.refresh();
        Ok(())
    }
    pub fn refresh(&mut self) {
        self.resources.clear();
        // Keep registry errors visible while rebuilding filesystem warnings.
        self.warnings.retain(|w| w.starts_with("Proje kaydı"));
        let mut visited = HashSet::new();
        for (root, provider) in [
            (".agents/skills", "Agents"),
            (".codex/skills", "Codex"),
            (".claude/skills", "Claude"),
            (".codex/plugins/cache", "Codex plugin"),
            (".claude/plugins/cache", "Claude plugin"),
        ] {
            self.walk(
                &self.home.join(root),
                Kind::Skill,
                true,
                provider,
                0,
                &mut visited,
            );
        }
        for root in [
            ".agents/AGENTS.md",
            ".codex/AGENTS.md",
            "AGENTS.md",
            ".claude/CLAUDE.md",
        ] {
            self.add_file(self.home.join(root), Kind::Instructions, true, "Global");
        }
        for (path, provider) in [
            (".codex/config.toml", "Codex"),
            (".claude.json", "Claude Code"),
            (
                ".config/Claude/claude_desktop_config.json",
                "Claude Desktop",
            ),
            (".cursor/mcp.json", "Cursor"),
            (".config/opencode/opencode.json", "OpenCode"),
        ] {
            self.add_file(self.home.join(path), Kind::Mcp, true, provider);
        }
        if let Some(project) = self.settings.active.clone() {
            let mut visited = HashSet::new();
            for (root, provider) in [
                (".agents/skills", "Agents"),
                (".claude/skills", "Claude"),
                (".codex/skills", "Codex"),
            ] {
                self.walk(
                    &project.join(root),
                    Kind::Skill,
                    false,
                    provider,
                    0,
                    &mut visited,
                );
            }
            self.walk(
                &project.join(".agents"),
                Kind::Instructions,
                false,
                "Agents",
                0,
                &mut HashSet::new(),
            );
            for root in [
                "AGENTS.md",
                "CLAUDE.md",
                ".claude/CLAUDE.md",
                ".codex/AGENTS.md",
            ] {
                self.add_file(project.join(root), Kind::Instructions, false, "Proje");
            }
            for (path, provider) in [
                (".mcp.json", "Claude"),
                (".cursor/mcp.json", "Cursor"),
                (".codex/config.toml", "Codex"),
                ("opencode.json", "OpenCode"),
            ] {
                self.add_file(project.join(path), Kind::Mcp, false, provider);
            }
        }
        self.walk(
            &self.home.join(".agents"),
            Kind::Instructions,
            true,
            "Agents",
            0,
            &mut HashSet::new(),
        );
        for source in self.settings.sources.clone() {
            if source.path.is_dir() {
                self.walk(
                    &source.path,
                    source.kind,
                    true,
                    "Özel kaynak",
                    0,
                    &mut HashSet::new(),
                );
            } else {
                self.add_file(source.path, source.kind, true, "Özel kaynak");
            }
        }
        self.resources.sort_by_key(|a| a.name.to_lowercase());
    }
    fn walk(
        &mut self,
        path: &Path,
        kind: Kind,
        global: bool,
        provider: &str,
        depth: usize,
        visited: &mut HashSet<PathBuf>,
    ) {
        if depth > 10 {
            self.warnings
                .push(format!("Tarama derinliği aşıldı: {}", path.display()));
            return;
        }
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return,
        };
        if !visited.insert(canonical) {
            return;
        }
        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(e) => {
                self.warnings.push(format!("{}: {e}", path.display()));
                return;
            }
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if kind == Kind::Instructions
                    && ["skills", "node_modules", ".git"].contains(&name.as_str())
                {
                    continue;
                }
                self.walk(&p, kind, global, provider, depth + 1, visited);
            } else if (kind == Kind::Skill && (name == "SKILL.md" || name == "SKILL.md.disabled"))
                || (kind == Kind::Instructions && p.extension().is_some_and(|x| x == "md"))
            {
                self.add_file(p, kind, global, provider);
            }
        }
    }
    fn add_file(&mut self, path: PathBuf, kind: Kind, global: bool, provider: &str) {
        if !path.is_file() || self.resources.iter().any(|r| r.path == path) {
            return;
        }
        match read_document(&path) {
            Ok(raw) => {
                let fallback = if kind == Kind::Skill {
                    path.parent().and_then(Path::file_name)
                } else {
                    path.file_name()
                }
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
                let (name, description) = if kind == Kind::Skill {
                    skill_metadata(&raw, &fallback)
                } else if kind == Kind::Mcp {
                    let desc = match mcp_names(&path, &raw) {
                        Ok(names) => {
                            if names.is_empty() {
                                "Henüz MCP sunucusu tanımlanmamış".into()
                            } else {
                                format!("{} sunucu: {}", names.len(), names.join(", "))
                            }
                        }
                        Err(e) => format!("Yapılandırma hatası: {e}"),
                    };
                    (format!("{provider} · {fallback}"), desc)
                } else {
                    (
                        fallback,
                        raw.lines()
                            .find(|l| !l.trim().is_empty())
                            .unwrap_or("")
                            .trim_start_matches('#')
                            .trim()
                            .chars()
                            .take(140)
                            .collect(),
                    )
                };
                let enabled = !path.to_string_lossy().ends_with(".disabled");
                self.resources.push(Resource {
                    name,
                    description,
                    path,
                    kind,
                    global,
                    provider: provider.into(),
                    enabled,
                });
            }
            Err(e) => self.warnings.push(e),
        }
    }
    pub fn target(
        &self,
        kind: Kind,
        global: bool,
        name: &str,
        provider: &str,
    ) -> Result<PathBuf, String> {
        let root = if global {
            self.home.clone()
        } else {
            self.settings
                .active
                .clone()
                .ok_or("Önce bir proje ekleyin.")?
        };
        match kind {
            Kind::Skill => {
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                {
                    return Err("Skill adı yalnızca harf, rakam, - ve _ içerebilir.".into());
                }
                Ok(root.join(".agents/skills").join(name).join("SKILL.md"))
            }
            Kind::Instructions => Ok(root.join(if global {
                ".agents/AGENTS.md"
            } else {
                "AGENTS.md"
            })),
            Kind::Mcp => Ok(root.join(match (provider, global) {
                ("Codex", _) => ".codex/config.toml",
                ("Cursor", _) => ".cursor/mcp.json",
                ("Claude", true) => ".config/Claude/claude_desktop_config.json",
                _ => ".mcp.json",
            })),
        }
    }
}
pub fn expand_path(raw: &str, home: &Path) -> PathBuf {
    if raw.trim() == "~" {
        home.into()
    } else if let Some(rest) = raw.trim().strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(raw.trim())
    }
}
pub fn read_document(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 2 * 1024 * 1024 {
        return Err(format!("Dosya 2 MB sınırını aşıyor: {}", path.display()));
    }
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
pub fn skill_metadata(raw: &str, fallback: &str) -> (String, String) {
    let mut name = fallback.to_string();
    let mut description = String::new();
    if let Some(rest) = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
    {
        for line in rest.lines().take_while(|l| *l != "---") {
            if let Some(s) = line.strip_prefix("name:") {
                name = s.trim().trim_matches(['\'', '"']).to_string();
            }
            if let Some(s) = line.strip_prefix("description:") {
                description = s.trim().trim_matches(['\'', '"']).to_string();
            }
        }
    }
    if description.is_empty() {
        description = "Yerel skill paketi".into();
    }
    (name, description)
}
pub fn mcp_names(path: &Path, raw: &str) -> Result<Vec<String>, String> {
    if path.extension().is_some_and(|e| e == "toml") {
        let doc = raw
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| e.to_string())?;
        let Some(servers) = doc.get("mcp_servers") else {
            return Ok(vec![]);
        };
        let table = servers
            .as_table_like()
            .ok_or("mcp_servers bir tablo olmalı")?;
        Ok(table.iter().map(|(name, _)| name.to_string()).collect())
    } else {
        let doc: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if !doc.is_object() {
            return Err("Yapılandırma bir JSON nesnesi olmalı".into());
        }
        let Some(servers) = doc.get("mcpServers").or_else(|| doc.get("mcp")) else {
            return Ok(vec![]);
        };
        let table = servers
            .as_object()
            .ok_or("MCP alanı bir JSON nesnesi olmalı")?;
        Ok(table.keys().cloned().collect())
    }
}
/// Compare with the loaded document, back up existing bytes, then atomically replace.
pub fn save_file(
    path: &Path,
    original: Option<&str>,
    content: &str,
    kind: Option<Kind>,
) -> Result<PathBuf, String> {
    if kind == Some(Kind::Mcp) {
        mcp_names(path, content)?;
    }
    let target = if path.is_symlink() {
        path.canonicalize().map_err(|e| e.to_string())?
    } else {
        path.to_path_buf()
    };
    let current = match fs::read_to_string(&target) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if current.as_deref() != original {
        return Err("Dosya dışarıdan değişti veya zaten var. Yenileyip tekrar açın; değişiklikler kaydedilmedi.".into());
    }
    let parent = target.parent().ok_or("Üst klasör bulunamadı")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let backup = target.with_file_name(format!(
        "{}.kozmos-{stamp}.bak",
        target.file_name().unwrap_or_default().to_string_lossy()
    ));
    if current.is_some() {
        fs::copy(&target, &backup).map_err(|e| format!("Yedek alınamadı: {e}"))?;
    }
    let tmp = parent.join(format!(".kozmos-{}-{stamp}.tmp", std::process::id()));
    let result = (|| -> Result<(), String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        if let Ok(meta) = fs::metadata(&target) {
            file.set_permissions(meta.permissions())
                .map_err(|e| e.to_string())?;
        }
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        // Recheck after backup and temporary write to catch concurrent edits.
        let latest = fs::read_to_string(&target).ok();
        if latest.as_deref() != original {
            return Err("Dosya kayıt sırasında değişti; tekrar açın.".into());
        }
        if original.is_none() {
            // A hard link publishes a new document without ever replacing an existing file.
            fs::hard_link(&tmp, &target).map_err(|e| e.to_string())?;
            let _ = fs::remove_file(&tmp);
        } else {
            fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result?;
    Ok(backup)
}
pub fn toggle_skill(resource: &Resource) -> Result<(), String> {
    if resource.kind != Kind::Skill {
        return Err("Yalnızca skill durumu değiştirilebilir".into());
    }
    let target = resource.path.with_file_name(if resource.enabled {
        "SKILL.md.disabled"
    } else {
        "SKILL.md"
    });
    if target.exists() {
        return Err("Hedef dosya zaten var; işlem yapılmadı.".into());
    }
    fs::rename(&resource.path, target).map_err(|e| e.to_string())
}
pub fn install_skill(resource: &Resource, project: &Path) -> Result<PathBuf, String> {
    let source = resource.path.parent().ok_or("Skill klasörü bulunamadı")?;
    let name = source.file_name().ok_or("Skill adı bulunamadı")?;
    let root = project.join(".agents/skills");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let target = root.join(name);
    if target.exists() {
        return Err("Bu skill projede zaten var.".into());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let staging = root.join(format!(".kozmos-install-{stamp}"));
    let result = copy_tree(source, &staging, 0).and_then(|()| {
        if !resource.enabled {
            fs::rename(staging.join("SKILL.md.disabled"), staging.join("SKILL.md"))
                .map_err(|e| e.to_string())?;
        }
        fs::rename(&staging, &target).map_err(|e| e.to_string())
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    Ok(target)
}
fn copy_tree(source: &Path, target: &Path, depth: usize) -> Result<(), String> {
    if depth > 20 {
        return Err("Skill klasörü çok derin".into());
    }
    fs::create_dir(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        if ft.is_symlink() {
            return Err(format!(
                "Paket içinde sembolik bağlantı var: {}",
                entry.path().display()
            ));
        }
        if entry.file_name().to_string_lossy().contains(".kozmos-") {
            continue;
        }
        if ft.is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()), depth + 1)?;
        } else if ft.is_file() {
            fs::copy(entry.path(), target.join(entry.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn fixture() -> (tempfile::TempDir, Workspace) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        fs::create_dir(&home).unwrap();
        let ws = Workspace::new(home, temp.path().join("state/workspaces.json"));
        (temp, ws)
    }
    #[test]
    fn recent_projects_reorder_without_duplicates_and_survive_restart() {
        let (temp, mut ws) = fixture();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        ws.add_project(first.to_str().unwrap()).unwrap();
        ws.add_project(second.to_str().unwrap()).unwrap();
        ws.add_project(first.join(".").to_str().unwrap()).unwrap();
        let loaded = Workspace::new(ws.home.clone(), ws.state_path.clone());
        assert_eq!(loaded.settings.projects, vec![first.clone(), second]);
        assert_eq!(loaded.settings.active, Some(first));
    }

    #[test]
    fn failed_project_open_preserves_active_project_and_recent_order() {
        let (temp, mut ws) = fixture();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        ws.add_project(first.to_str().unwrap()).unwrap();
        assert!(
            ws.add_project(temp.path().join("missing").to_str().unwrap())
                .is_err()
        );
        // Simulate a concurrent registry edit; failed persistence must not switch scope.
        fs::write(&ws.state_path, "{}").unwrap();
        assert!(ws.add_project(second.to_str().unwrap()).is_err());
        assert_eq!(ws.settings.projects, vec![first.clone()]);
        assert_eq!(ws.settings.active, Some(first));
    }

    #[test]
    fn discovery_separates_scopes_and_finds_plugins_and_agent_files() {
        let (temp, mut ws) = fixture();
        put(
            &ws.home.join(".agents/skills/review/SKILL.md"),
            "---\nname: review\ndescription: Review code\n---\n",
        );
        put(
            &ws.home
                .join(".codex/plugins/cache/provider/pkg/1/skills/demo/SKILL.md.disabled"),
            "# Demo",
        );
        put(&ws.home.join(".agents/reviewer.md"), "# Reviewer");
        put(
            &ws.home.join(".cursor/mcp.json"),
            r#"{"mcpServers":{"filesystem":{"command":"example"}}}"#,
        );
        let project = temp.path().join("project");
        put(&project.join(".agents/skills/local/SKILL.md"), "# Local");
        put(&project.join("AGENTS.md"), "# Rules");
        ws.add_project(project.to_str().unwrap()).unwrap();
        assert_eq!(
            ws.resources
                .iter()
                .filter(|r| r.kind == Kind::Skill)
                .count(),
            3
        );
        assert!(
            ws.resources
                .iter()
                .any(|r| r.name == "review" && r.global && r.description == "Review code")
        );
        assert!(
            ws.resources
                .iter()
                .any(|r| r.kind == Kind::Skill && !r.enabled)
        );
        assert!(
            ws.resources
                .iter()
                .any(|r| r.kind == Kind::Instructions && !r.global)
        );
        assert!(
            ws.resources
                .iter()
                .any(|r| r.description.contains("filesystem"))
        );
    }
    #[test]
    fn discovery_handles_symlink_cycles() {
        let (_temp, mut ws) = fixture();
        let root = ws.home.join(".agents/skills");
        put(&root.join("demo/SKILL.md"), "demo");
        std::os::unix::fs::symlink(&root, root.join("demo/cycle")).unwrap();
        ws.refresh();
        assert_eq!(ws.resources.len(), 1);
    }
    #[test]
    fn save_backs_up_bytes_preserves_permissions_and_rejects_stale_edits() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mcp.json");
        let original = "{\"mcpServers\": {}}\n";
        put(&path, original);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let edited = "{\"mcpServers\": {\"demo\": {\"command\": \"tool\"}}}\n";
        let backup = save_file(&path, Some(original), edited, Some(Kind::Mcp)).unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), original);
        assert_eq!(fs::read_to_string(&path).unwrap(), edited);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(save_file(&path, Some(original), "{}", Some(Kind::Mcp)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), edited);
    }
    #[test]
    fn invalid_configs_and_existing_new_files_are_never_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        put(
            &path,
            "title = 'keep'\n[mcp_servers.demo]\ncommand = 'tool'\n",
        );
        let original = fs::read_to_string(&path).unwrap();
        assert!(save_file(&path, Some(&original), "[bad", Some(Kind::Mcp)).is_err());
        assert!(save_file(&path, None, "", Some(Kind::Mcp)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert!(mcp_names(Path::new("mcp.json"), "[]").is_err());
        assert!(mcp_names(Path::new("mcp.json"), r#"{"mcpServers":[]}"#).is_err());
        assert_eq!(mcp_names(&path, &original).unwrap(), vec!["demo"]);
    }
    #[test]
    fn save_preserves_symlink_and_changes_its_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("rules.md");
        put(&target, "old");
        let link = temp.path().join("AGENTS.md");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        save_file(&link, Some("old"), "new", Some(Kind::Instructions)).unwrap();
        assert!(link.is_symlink());
        assert_eq!(fs::read_to_string(target).unwrap(), "new");
    }
    #[test]
    fn skill_install_copies_resources_and_toggle_is_reversible() {
        let (temp, mut ws) = fixture();
        let source = ws.home.join(".agents/skills/demo");
        put(&source.join("SKILL.md"), "---\nname: demo\n---\n");
        put(&source.join("scripts/run.py"), "print('ok')");
        ws.refresh();
        let resource = ws.resources[0].clone();
        let project = temp.path().join("project");
        let target = install_skill(&resource, &project).unwrap();
        assert!(target.join("scripts/run.py").exists());
        assert!(install_skill(&resource, &project).is_err());
        toggle_skill(&resource).unwrap();
        assert!(!resource.path.exists());
        let mut disabled = resource;
        disabled.enabled = false;
        disabled.path = source.join("SKILL.md.disabled");
        toggle_skill(&disabled).unwrap();
        assert!(source.join("SKILL.md").exists());
    }
    #[test]
    fn failed_install_rolls_back_instead_of_leaving_partial_package() {
        let (temp, mut ws) = fixture();
        let source = ws.home.join(".agents/skills/demo");
        put(&source.join("SKILL.md"), "demo");
        std::os::unix::fs::symlink("/tmp", source.join("outside")).unwrap();
        ws.refresh();
        let project = temp.path().join("project");
        assert!(install_skill(&ws.resources[0], &project).is_err());
        assert_eq!(
            fs::read_dir(project.join(".agents/skills"))
                .unwrap()
                .count(),
            0
        );
    }
    #[test]
    fn registry_custom_sources_and_project_switch_survive_restart() {
        let (temp, mut ws) = fixture();
        let project = temp.path().join("project");
        fs::create_dir(&project).unwrap();
        ws.add_project(project.to_str().unwrap()).unwrap();
        let extra = temp.path().join("extra/custom.json");
        put(&extra, r#"{"mcpServers":{"demo":{}}}"#);
        ws.add_source(extra.to_str().unwrap(), Kind::Mcp).unwrap();
        ws.settings
            .project_names
            .insert(project.to_string_lossy().into_owned(), "Mission".into());
        ws.save_settings().unwrap();
        let loaded = Workspace::new(ws.home.clone(), ws.state_path.clone());
        assert_eq!(
            loaded
                .settings
                .project_names
                .get(&project.to_string_lossy().into_owned())
                .map(String::as_str),
            Some("Mission")
        );
        assert_eq!(loaded.settings.active, Some(project));
        assert_eq!(loaded.settings.sources.len(), 1);
        assert!(loaded.resources.iter().any(|r| r.provider == "Özel kaynak"));
        assert!(ws.target(Kind::Skill, true, "../escape", "").is_err());
    }
}
