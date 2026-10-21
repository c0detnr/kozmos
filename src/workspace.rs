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
    #[serde(skip)]
    pub active: Option<PathBuf>,
}
pub struct Workspace {
    root: PathBuf,
    pub settings: Settings,
    pub resources: Vec<Resource>,
    pub warnings: Vec<String>,
    state_path: PathBuf,
    state_original: Option<String>,
}
impl Workspace {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("Çalışma dizini okunamadı: {e}"))?;
        if !root.is_dir() {
            return Err("Çalışma dizini bir klasör olmalı.".into());
        }
        let state_path = root.join(".kozmos/workspace.json");
        check_path(&root, &state_path)?;
        let mut warnings = Vec::new();
        let raw = match fs::read_to_string(&state_path) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                warnings.push(format!("Proje kaydı okunamadı: {e}"));
                None
            }
        };
        let mut settings: Settings = raw
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .unwrap_or_else(|e| {
                warnings.push(format!("Proje kaydı geçersiz: {e}"));
                Some(Settings::default())
            })
            .unwrap_or_default();
        settings.active = Some(root.clone());
        let mut ws = Self {
            root,
            settings,
            resources: vec![],
            warnings,
            state_path,
            state_original: raw,
        };
        ws.refresh();
        Ok(ws)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn check_path(&self, path: &Path) -> Result<(), String> {
        check_path(&self.root, path)
    }
    pub fn save_settings(&mut self) -> Result<(), String> {
        self.check_path(&self.state_path)?;
        let raw = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
        save_file(&self.state_path, self.state_original.as_deref(), &raw, None)?;
        self.state_original = Some(raw);
        Ok(())
    }
    pub fn add_source(&mut self, raw: &str, kind: Kind) -> Result<(), String> {
        let path = self
            .root
            .join(raw.trim())
            .canonicalize()
            .map_err(|e| e.to_string())?;
        self.check_path(&path)?;
        if kind == Kind::Mcp {
            if !path.is_file() {
                return Err("MCP için bir JSON veya TOML dosyası seçin.".into());
            }
            mcp_names(&path, &read_document(&path)?)?;
        }
        let previous = self.settings.clone();
        if !self
            .settings
            .sources
            .iter()
            .any(|s| s.path == path && s.kind == kind)
        {
            self.settings.sources.push(Source { path, kind });
        }
        if let Err(error) = self.save_settings() {
            self.settings = previous;
            return Err(error);
        }
        self.refresh();
        Ok(())
    }
    pub fn refresh(&mut self) {
        self.resources.clear();
        // Keep registry errors visible while rebuilding filesystem warnings.
        self.warnings.retain(|w| w.starts_with("Proje kaydı"));
        {
            let project = self.root.clone();
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
        for source in self.settings.sources.clone() {
            if let Err(error) = self.check_path(&source.path) {
                self.warnings.push(error);
                continue;
            }
            if source.path.is_dir() {
                self.walk(
                    &source.path,
                    source.kind,
                    false,
                    "Özel kaynak",
                    0,
                    &mut HashSet::new(),
                );
            } else {
                self.add_file(source.path, source.kind, false, "Özel kaynak");
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
        if let Err(error) = self.check_path(path) {
            self.warnings.push(error);
            return;
        }
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
        if let Err(error) = self.check_path(&path) {
            self.warnings.push(error);
            return;
        }
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
        if global {
            return Err("Kaynaklar yalnızca çalışma dizininde oluşturulabilir.".into());
        }
        let root = &self.root;
        let target = match kind {
            Kind::Skill => {
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                {
                    return Err("Skill adı yalnızca harf, rakam, - ve _ içerebilir.".into());
                }
                root.join(".agents/skills").join(name).join("SKILL.md")
            }
            Kind::Instructions => root.join("AGENTS.md"),
            Kind::Mcp => root.join(match provider {
                "Codex" => ".codex/config.toml",
                "Cursor" => ".cursor/mcp.json",
                _ => ".mcp.json",
            }),
        };
        self.check_path(&target)?;
        Ok(target)
    }
}

// Resolve existing ancestors as well, so new files cannot escape through a linked directory.
pub(crate) fn check_path(root: &Path, path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || !path.starts_with(root)
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(format!("Yol çalışma dizininin dışında: {}", path.display()));
    }
    let mut ancestor = path;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                let resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
                if !resolved.starts_with(root) {
                    return Err(format!("Yol çalışma dizininin dışında: {}", path.display()));
                }
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| "Geçersiz kaynak yolu.".to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
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
#[cfg(test)]
mod tests {
    use super::*;
    fn put(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn fixture() -> (tempfile::TempDir, Workspace) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        fs::create_dir(&root).unwrap();
        let ws = Workspace::new(root).unwrap();
        (temp, ws)
    }
    #[test]
    fn launch_root_overrides_stored_project_and_only_reads_local_resources() {
        let (temp, mut ws) = fixture();
        let other = temp.path().join("other");
        put(&other.join("AGENTS.md"), "# Other");
        put(&ws.root.join("AGENTS.md"), "# Local");
        put(
            &ws.root.join(".agents/skills/demo/SKILL.md"),
            "---\nname: demo\n---\n",
        );
        put(
            &ws.root.join(".cursor/mcp.json"),
            r#"{"mcpServers":{"demo":{}}}"#,
        );
        put(
            &ws.state_path,
            &serde_json::json!({
                "active": other, "projects": [other],
                "sources": [{"path": other.join("AGENTS.md"), "kind": "Instructions"}]
            })
            .to_string(),
        );
        ws = Workspace::new(ws.root.clone()).unwrap();
        assert_eq!(ws.settings.active.as_ref(), Some(&ws.root));
        assert_eq!(ws.resources.len(), 3);
        assert!(
            ws.resources
                .iter()
                .all(|r| !r.global && r.path.starts_with(&ws.root))
        );
        assert!(!ws.warnings.is_empty());
        ws.refresh();
        assert_eq!(ws.resources.len(), 3);
    }
    #[test]
    fn discovery_blocks_external_symlinks_and_handles_internal_cycles() {
        let (temp, mut ws) = fixture();
        let root = ws.root.join(".agents/skills");
        put(&root.join("demo/SKILL.md"), "demo");
        let external = temp.path().join("external");
        put(&external.join("SKILL.md"), "outside");
        std::os::unix::fs::symlink(&root, root.join("demo/cycle")).unwrap();
        std::os::unix::fs::symlink(&external, root.join("external")).unwrap();
        std::os::unix::fs::symlink(external.join("SKILL.md"), ws.root.join("AGENTS.md")).unwrap();
        ws.refresh();
        assert_eq!(ws.resources.len(), 1);
        assert!(!ws.warnings.is_empty());
    }
    #[test]
    fn targets_and_custom_sources_stay_in_launch_directory() {
        let (temp, mut ws) = fixture();
        let outside = temp.path().join("outside.md");
        put(&outside, "outside");
        assert!(
            ws.add_source(outside.to_str().unwrap(), Kind::Instructions)
                .is_err()
        );
        assert!(ws.add_source("../outside.md", Kind::Instructions).is_err());
        assert!(ws.target(Kind::Skill, true, "demo", "").is_err());
        assert!(ws.target(Kind::Skill, false, "../escape", "").is_err());
        std::os::unix::fs::symlink(temp.path(), ws.root.join(".codex")).unwrap();
        assert!(ws.target(Kind::Mcp, false, "", "Codex").is_err());
        assert!(ws.check_path(&ws.root.join(".codex/new/file.md")).is_err());
        put(&ws.root.join("rules/custom.md"), "# Local rules");
        ws.add_source("rules/custom.md", Kind::Instructions)
            .unwrap();
        ws.settings
            .project_names
            .insert("name".into(), "Demo".into());
        ws.save_settings().unwrap();
        let loaded = Workspace::new(ws.root.clone()).unwrap();
        assert_eq!(loaded.settings.project_names.get("name").unwrap(), "Demo");
        assert_eq!(loaded.resources.len(), 1);
        assert_eq!(loaded.resources[0].provider, "Özel kaynak");
        let other = temp.path().join("second");
        fs::create_dir(&other).unwrap();
        let second = Workspace::new(other.clone()).unwrap();
        assert_eq!(second.root(), other);
        assert!(second.settings.sources.is_empty());
        assert!(second.settings.project_names.is_empty());
    }
    #[test]
    fn settings_cannot_follow_external_symlink() {
        let (temp, ws) = fixture();
        std::os::unix::fs::symlink(temp.path(), ws.root.join(".kozmos")).unwrap();
        assert!(Workspace::new(ws.root.clone()).is_err());
    }
    #[test]
    fn rejects_missing_or_non_directory_root() {
        let temp = tempfile::tempdir().unwrap();
        assert!(Workspace::new(temp.path().join("missing")).is_err());
        let file = temp.path().join("file");
        put(&file, "file");
        assert!(Workspace::new(file).is_err());
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
    fn local_skill_toggle_is_reversible() {
        let (_temp, mut ws) = fixture();
        let source = ws.root.join(".agents/skills/demo/SKILL.md");
        put(&source, "demo");
        ws.refresh();
        toggle_skill(&ws.resources[0]).unwrap();
        ws.refresh();
        assert!(!ws.resources[0].enabled);
        toggle_skill(&ws.resources[0]).unwrap();
        assert!(source.exists());
    }
}
