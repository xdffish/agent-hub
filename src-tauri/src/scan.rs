use crate::{app::Hub, files::*, model::*};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub const AGENTS: [&str; 4] = ["claude", "codex", "cursor", "antigravity"];
pub fn display_name(agent: &str) -> &str {
    match agent {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "cursor" => "Cursor",
        _ => "Antigravity",
    }
}
pub fn guide(agent: &str) -> &str {
    match agent {
        "claude" => "https://code.claude.com/docs/en/discover-plugins",
        "codex" => "https://learn.chatgpt.com/docs/developer-commands",
        "cursor" => "https://cursor.com/docs/plugins",
        _ => "https://antigravity.google/docs/plugins",
    }
}
pub fn codex_home(home: &Path) -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"))
}

pub fn skill_roots(home: &Path, project: Option<&str>) -> Vec<(PathBuf, Vec<String>)> {
    let base = project
        .map(PathBuf::from)
        .unwrap_or_else(|| home.to_path_buf());
    let group = |a: &[&str]| a.iter().map(|s| s.to_string()).collect();
    let mut roots = vec![
        (
            base.join(".agents/skills"),
            group(if project.is_some() {
                &["codex", "cursor", "antigravity"]
            } else {
                &["codex", "cursor"]
            }),
        ),
        (base.join(".claude/skills"), group(&["claude", "cursor"])),
        (base.join(".cursor/skills"), group(&["cursor"])),
        (
            if project.is_some() {
                base.join(".codex/skills")
            } else {
                codex_home(home).join("skills")
            },
            group(
                if project.is_none() && codex_home(home) != home.join(".codex") {
                    &["codex"]
                } else {
                    &["codex", "cursor"]
                },
            ),
        ),
    ];
    if project.is_none() && codex_home(home) != home.join(".codex") {
        roots.push((home.join(".codex/skills"), group(&["cursor"])));
    }
    if project.is_some() {
        roots.push((base.join(".agent/skills"), group(&["antigravity"])));
    } else {
        for relative in [".gemini/config/skills", ".gemini/antigravity/skills"] {
            roots.push((base.join(relative), group(&["antigravity"])));
        }
    }
    roots
}

pub fn skill_metadata(path: &Path) -> Result<(String, String)> {
    let text = fs::read_to_string(path.join("SKILL.md"))
        .map_err(|_| "Skill 目录必须包含可读的 SKILL.md")?;
    let fallback = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut name = fallback;
    let mut description = String::new();
    let mut lines = text.lines().peekable();
    if lines.next().map(str::trim) == Some("---") {
        let mut count = 0;
        while let Some(line) = lines.next() {
            count += 1;
            if count > 150 || line.trim() == "---" {
                break;
            }
            if let Some(v) = line.strip_prefix("name:") {
                name = v.trim().trim_matches(['\'', '"']).to_string();
            }
            if let Some(v) = line.strip_prefix("description:") {
                let v = v.trim();
                if matches!(v, ">" | ">-" | ">+" | "|" | "|-" | "|+") {
                    let separator = if v.starts_with('|') { "\n" } else { " " };
                    let mut parts = Vec::new();
                    while let Some(next) = lines.peek() {
                        if !next.trim().is_empty()
                            && !next.starts_with(' ')
                            && !next.starts_with('\t')
                        {
                            break;
                        }
                        parts.push(lines.next().unwrap_or_default().trim().to_string());
                    }
                    description = parts.join(separator).trim().to_string();
                } else {
                    description = v.trim_matches(['\'', '"']).to_string();
                }
            }
        }
    }
    if description.is_empty() || description == ">" || description == "|" {
        description = "查看 SKILL.md 了解用途与依赖".into();
    }
    Ok((name, description.chars().take(1200).collect()))
}

pub fn parse_config(path: &Path, agent: &str) -> Result<Value> {
    let meta = fs::metadata(path).map_err(|_| "无法读取配置文件")?;
    if meta.len() > MAX_BYTES {
        return Err("配置文件超过 20 MB".into());
    }
    let text = fs::read_to_string(path).map_err(|_| "配置不是可读的 UTF-8 文本")?;
    if agent == "codex" {
        toml_edit::de::from_str(&text).map_err(|_| "TOML 格式错误，未读取配置值".into())
    } else {
        serde_json::from_str(&text)
            .map_err(|_| "需要标准 JSON；发现注释或语法错误时不会自动重写".into())
    }
}
pub fn servers<'a>(value: &'a Value, agent: &str) -> Option<&'a serde_json::Map<String, Value>> {
    value
        .get(if agent == "codex" {
            "mcp_servers"
        } else {
            "mcpServers"
        })?
        .as_object()
}

pub fn run(hub: &Hub) -> Result<Scan> {
    let home = hub.home()?;
    let settings = hub.settings()?;
    let mut resources: Vec<Resource> = vec![];
    let mut warnings = vec![];
    let mut fingerprints = SkillFingerprints::default();
    let mut library_index = BTreeMap::new();
    let mut source_index: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for skill in &settings.library {
        // Source identity survives edits; a content match alone cannot track an imported source.
        if Path::new(&skill.source).is_absolute() {
            if let Ok(source) = Path::new(&skill.source).join(&skill.subdir).canonicalize() {
                source_index
                    .entry(source)
                    .or_default()
                    .push(skill.id.clone());
            }
        }
        match hub
            .library_path(skill)
            .and_then(|path| fingerprints.get(&path))
        {
            Ok(fp) => {
                library_index.entry(fp).or_insert_with(|| skill.id.clone());
            }
            Err(error) => warnings.push(format!("Agent Hub 管理版本 {}：{error}", skill.name)),
        }
    }
    let agents = AGENTS
        .iter()
        .map(|a| {
            let binary = match *a {
                "claude" => "claude",
                "codex" => "codex",
                "cursor" => "cursor",
                _ => "agy",
            };
            let mut evidence = vec![];
            for path in crate::platform::app_candidates(a, &home) {
                if path.exists() {
                    evidence.push(path_string(&path));
                }
            }
            // A CLI path is evidence only; no executable is run during scanning.
            if let Some(paths) = std::env::var_os("PATH") {
                for p in std::env::split_paths(&paths) {
                    if let Some(path) = crate::platform::executable_names(binary)
                        .iter()
                        .map(|n| p.join(n))
                        .find(|p| p.is_file())
                    {
                        evidence.push(path_string(&path));
                        break;
                    }
                }
            }
            Agent {
                id: a.to_string(),
                name: display_name(a).into(),
                detected: !evidence.is_empty(),
                evidence: if evidence.is_empty() {
                    "未在标准应用目录或当前 PATH 发现；配置仍可盘点".into()
                } else {
                    evidence.join(" · ")
                },
                guide: guide(a).into(),
            }
        })
        .collect();
    let scopes: Vec<Option<&str>> = std::iter::once(None)
        .chain(settings.projects.iter().map(|p| Some(p.as_str())))
        .collect();
    for project in scopes {
        if let Some(path) = project {
            if let Err(error) = ensure_no_symlink_ancestors(Path::new(path)) {
                warnings.push(format!("跳过已改变授权路径的项目 {path}：{error}"));
                continue;
            }
        }
        let scope = project.unwrap_or("global").to_string();
        for (root, agent_ids) in skill_roots(&home, project) {
            if !root.exists() {
                continue;
            }
            let mut seen = BTreeSet::new();
            for item in walkdir::WalkDir::new(&root)
                .max_depth(5)
                .follow_links(true)
                .into_iter()
                .take(6000)
            {
                let item = match item {
                    Ok(v) => v,
                    Err(_) => {
                        warnings.push(format!("无法读取部分 Skill：{}", path_string(&root)));
                        continue;
                    }
                };
                if item.file_name() != "SKILL.md" || !item.file_type().is_file() {
                    continue;
                }
                let folder = item.path().parent().unwrap_or(&root);
                if !seen.insert(folder.to_path_buf()) {
                    continue;
                }
                let resolved = match fs::canonicalize(folder) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let (name, description) = match skill_metadata(folder) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let fp = match fingerprints.get(&resolved) {
                    Ok(fp) => fp,
                    Err(e) => {
                        warnings.push(format!("{}：{e}", path_string(folder)));
                        hash(&fs::read(item.path()).unwrap_or_default())
                    }
                };
                let path = path_string(folder);
                let rid = hash(format!("skill:{path}").as_bytes());
                let changed = settings
                    .observations
                    .get(&rid)
                    .is_some_and(|old| old != &fp);
                let library_id = settings
                    .installations
                    .get(&path)
                    .cloned()
                    .or_else(|| {
                        source_index
                            .get(&resolved)
                            .filter(|ids| ids.len() == 1)
                            .map(|ids| ids[0].clone())
                    })
                    .or_else(|| library_index.get(&fp).cloned());
                resources.push(Resource {id:rid,kind:"skill".into(),name,description,path,canonical:path_string(&resolved),is_link:fs::symlink_metadata(folder).is_ok_and(|m| crate::platform::is_link(&m)),scope:scope.clone(),agents:agent_ids.clone(),fingerprint:fp,changed,library_id,detail:"已发现文件；具体加载状态、依赖和平台专属指令需在 Agent 中确认。旧兼容目录按候选来源展示。".into()});
            }
        }
        for agent in AGENTS {
            let target = Target {
                agent: agent.into(),
                project: project.map(str::to_string),
            };
            let path = hub.target(&target, false)?;
            if path.exists() {
                match parse_config(&path, agent) {
                    Ok(value) => {
                        append_mcp(&mut resources, &settings, &path, &scope, agent, &value);
                        if agent == "claude" && project.is_none() {
                            if let Some(projects) = value.get("projects").and_then(Value::as_object)
                            {
                                for p in &settings.projects {
                                    if let Some(v) = projects.get(p) {
                                        append_mcp(
                                            &mut resources,
                                            &settings,
                                            &path,
                                            &format!("{p} (Claude local)"),
                                            agent,
                                            v,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => warnings.push(format!("{}：{e}", path_string(&path))),
                }
            }
        }
        let base = project.map(PathBuf::from).unwrap_or_else(|| home.clone());
        for (agent, relative) in [
            ("claude", ".claude/plugins"),
            ("codex", ".codex/plugins"),
            ("cursor", ".cursor/plugins"),
            (
                "antigravity",
                if project.is_some() {
                    ".agents/plugins"
                } else {
                    ".gemini/config/plugins"
                },
            ),
        ] {
            let root = if agent == "codex" && project.is_none() {
                codex_home(&home).join("plugins")
            } else {
                base.join(relative)
            };
            if !root.exists() {
                continue;
            }
            for item in walkdir::WalkDir::new(&root)
                .max_depth(6)
                .follow_links(false)
                .into_iter()
                .take(6000)
                .flatten()
            {
                if item.file_name() != "plugin.json" || !item.file_type().is_file() {
                    continue;
                }
                let path = path_string(item.path());
                let bytes = match fs::read(item.path()) {
                    Ok(b) if b.len() < 1024 * 1024 => b,
                    _ => continue,
                };
                let v: Value = match serde_json::from_slice(&bytes) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let name = v
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("未命名插件")
                    .to_string();
                let rid = hash(format!("plugin:{path}").as_bytes());
                let fp = hash(&bytes);
                resources.push(Resource{id:rid.clone(),kind:"plugin".into(),name,description:v.get("description").and_then(Value::as_str).unwrap_or("原生插件清单").chars().take(300).collect(),path:path.clone(),canonical:path,is_link:false,scope:scope.clone(),agents:vec![agent.into()],fingerprint:fp.clone(),changed:settings.observations.get(&rid).is_some_and(|old|old!=&fp),library_id:None,detail:"在插件目录发现 manifest，可能是缓存或开发版本；不代表插件已启用。安装、启用、卸载请使用原生管理入口。".into()});
            }
        }
    }
    warnings.push(
        "插件扫描仅覆盖已知本地目录中的 manifest，未读取内部安装数据库；数量不代表全部启用插件。"
            .into(),
    );
    resources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(Scan {
        platform: std::env::consts::OS.into(),
        agents,
        resources,
        library: settings.library,
        projects: settings.projects,
        warnings,
        scanned_at: now(),
        data_dir: path_string(&hub.root()?),
    })
}
fn append_mcp(
    out: &mut Vec<Resource>,
    settings: &Settings,
    path: &Path,
    scope: &str,
    agent: &str,
    value: &Value,
) {
    if let Some(servers) = servers(value, agent) {
        for (name, config) in servers {
            let path = path_string(path);
            let rid = hash(format!("mcp:{path}:{scope}:{name}").as_bytes());
            let fp = hash(&serde_json::to_vec(config).unwrap_or_default());
            let transport = if config.get("command").is_some() {
                "stdio"
            } else {
                "HTTP / OAuth"
            };
            let keys = config
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            out.push(Resource {
                id: rid.clone(),
                kind: "mcp".into(),
                name: name.clone(),
                description: format!("{transport} · 已发现配置，连接尚未检查"),
                path: path.clone(),
                canonical: path,
                is_link: false,
                scope: scope.into(),
                agents: vec![agent.into()],
                fingerprint: fp.clone(),
                changed: settings
                    .observations
                    .get(&rid)
                    .is_some_and(|old| old != &fp),
                library_id: None,
                detail: format!(
                    "配置字段：{keys}\n所有配置值均留在 Rust 后端，不在扫描结果中暴露凭据。"
                ),
            });
        }
    }
}

pub fn observations(scan: &Scan) -> BTreeMap<String, String> {
    scan.resources
        .iter()
        .map(|r| (r.id.clone(), r.fingerprint.clone()))
        .collect()
}

#[derive(Default)]
pub(crate) struct SkillFingerprints {
    cache: BTreeMap<PathBuf, Result<String>>,
    #[cfg(test)]
    pub reads: usize,
}
impl SkillFingerprints {
    pub(crate) fn get(&mut self, path: &Path) -> Result<String> {
        let canonical = path.canonicalize().map_err(|_| "资源目录不存在")?;
        if let Some(result) = self.cache.get(&canonical) {
            return result.clone();
        }
        #[cfg(test)]
        {
            self.reads += 1;
        }
        let result = snapshot(&canonical).map(|s| fingerprint(&s));
        self.cache.insert(canonical, result.clone());
        result
    }
}
