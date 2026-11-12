use crate::{
    access::{self, PathPurpose},
    app::Hub,
    files::*,
    model::*,
    scan as scanner,
};
use serde_json::{json, Value};
use std::{fs, path::Path, time::Duration};
use tauri::{AppHandle, State};

#[tauri::command]
#[specta::specta]
pub fn scan(hub: State<Hub>, accept_changes: bool) -> Result<Scan> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let mut result = scanner::run(&hub)?;
    let mut settings = hub.settings()?;
    if settings.observations.is_empty() || accept_changes {
        settings.observations = scanner::observations(&result);
        atomic_write(
            &hub.root()?.join("observations.json"),
            &serde_json::to_vec(&settings.observations).map_err(|_| "保存盘点基准失败")?,
            false,
        )?;
        for r in &mut result.resources {
            r.changed = false;
        }
    }
    Ok(result)
}
#[tauri::command]
#[specta::specta]
pub fn add_project(hub: State<Hub>, path: String) -> Result<()> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let p = fs::canonicalize(checked_path(&path)?).map_err(|_| "项目目录不存在")?;
    hub.require_grant(PathPurpose::Project, &p)?;
    if !p.is_dir() {
        return Err("请选择目录".into());
    }
    let mut s = hub.settings()?;
    let p = path_string(&p);
    if !s.projects.contains(&p) {
        s.projects.push(p);
        hub.save(&s)?;
    }
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub fn remove_project(hub: State<Hub>, path: String) -> Result<()> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let mut s = hub.settings()?;
    s.projects.retain(|p| p != &path);
    hub.save(&s)
}
async fn git(args: Vec<String>) -> Result<String> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "protocol.file.allow=never",
    ])
    .args(args)
    .env("GIT_TERMINAL_PROMPT", "0")
    .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(45), cmd.output())
        .await
        .map_err(|_| "Git 操作超时（45 秒）")?
        .map_err(|_| "无法启动 Git，请确认已安装")?;
    if !output.status.success() {
        return Err(
            "Git 操作失败，请检查仓库地址、访问权限和网络。未输出可能含凭据的远端错误。".into(),
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
fn safe_subdir(value: &str) -> Result<()> {
    if !value.is_empty()
        && (Path::new(value).is_absolute()
            || Path::new(value)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_))))
    {
        return Err("仓库内目录必须为不含 .. 的相对路径".into());
    }
    Ok(())
}
async fn get_source(
    app: &AppHandle,
    hub: &Hub,
    source: &str,
    subdir: &str,
) -> Result<(Snapshot, String, String, String)> {
    safe_subdir(subdir)?;
    if source.starts_with("https://") {
        let url = reqwest::Url::parse(source).map_err(|_| "仓库 URL 无效")?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("仓库链接不能包含凭据、查询参数或片段；请用单独的仓库内目录字段".into());
        }
        access::confirm(app,"拉取 Skill 仓库",format!("将通过 Git 从以下 HTTPS 仓库下载内容：\n{source}\n\n仅下载到应用临时目录，不运行 Skill 附带脚本。Git 可能使用你已配置的凭据助手。" )).await?;
        let temp = hub.root()?.join("git").join(id());
        fs::create_dir_all(temp.parent().unwrap()).map_err(|_| "创建临时目录失败")?;
        let result = async {
            git(vec![
                "clone".into(),
                "--depth=1".into(),
                "--".into(),
                source.into(),
                path_string(&temp),
            ])
            .await?;
            let revision = git(vec![
                "-C".into(),
                path_string(&temp),
                "rev-parse".into(),
                "HEAD".into(),
            ])
            .await?;
            let path = temp
                .join(subdir)
                .canonicalize()
                .map_err(|_| "仓库内 Skill 目录不存在")?;
            if !path.starts_with(temp.canonicalize().map_err(|_| "临时目录不可用")?) {
                return Err("仓库内目录越过来源边界".into());
            }
            let (name, description) = scanner::skill_metadata(&path)?;
            let content = source_snapshot(&path)?;
            validate_skill(&content)?;
            Ok((content, revision, name, description))
        }
        .await;
        if temp.exists() {
            let _ = fs::remove_dir_all(&temp);
        }
        result
    } else {
        let path = fs::canonicalize(checked_path(source)?)
            .map_err(|_| "本地 Skill 目录不存在")?
            .join(subdir)
            .canonicalize()
            .map_err(|_| "Skill 子目录不存在")?;
        if hub.require_grant(PathPurpose::Source, &path).is_err() {
            let inventory = scanner::run(hub)?;
            if !inventory
                .resources
                .iter()
                .any(|r| r.kind == "skill" && Path::new(&r.canonical) == path)
            {
                return Err("本地来源未授权，请使用原生目录选择器".into());
            }
        }
        let (name, description) = scanner::skill_metadata(&path)?;
        let content = source_snapshot(&path)?;
        validate_skill(&content)?;
        let revision = fingerprint(&content);
        Ok((content, revision, name, description))
    }
}
fn validate_skill(content: &Snapshot) -> Result<()> {
    validate_snapshot(content)?;
    if !content.exists
        || !content.directory
        || !content.entries.iter().any(|e| e.path == "SKILL.md")
    {
        return Err("目录根部必须包含 SKILL.md".into());
    }
    for e in &content.entries {
        let name = Path::new(&e.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if name == ".env"
            || name.starts_with(".env.")
            || name.ends_with(".pem")
            || name.ends_with(".key")
            || name == "credentials.json"
        {
            return Err(
                "Skill 含疑似凭据文件（.env / .pem / .key / credentials.json），请移除后再导入"
                    .into(),
            );
        }
    }
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub async fn import_skill(
    app: AppHandle,
    hub: State<'_, Hub>,
    source: String,
    subdir: String,
) -> Result<PlanView> {
    let _operation = hub.operation();
    let (content, revision, name, description) = get_source(&app, &hub, &source, &subdir).await?;
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    valid_name(&name)?;
    let mut s = hub.settings()?;
    let sid = hash(format!("{source}#{subdir}").as_bytes());
    if s.library.iter().any(|x| x.id == sid) {
        return Err("此来源已纳入 Agent Hub 管理，请使用检查更新".into());
    }
    let folder = hub.root()?.join("library").join(&sid);
    s.library.push(Skill {
        id: sid,
        name: name.clone(),
        description,
        source,
        revision,
        pinned: false,
        folder: path_string(&folder),
        subdir,
    });
    hub.queue(
        format!("导入 Skill · {name}"),
        vec![
            Change {
                path: path_string(&folder),
                before: snapshot(&folder)?,
                after: content,
                sensitive: false,
            },
            hub.settings_change(&s)?,
        ],
        vec![
            "仅导入 Agent Hub 管理数据，尚未安装到任何 Agent。导入过程不会执行 Skill 脚本。".into(),
        ],
    )
}
#[tauri::command]
#[specta::specta]
pub fn deploy_skill(hub: State<Hub>, skill_id: String, targets: Vec<Target>) -> Result<PlanView> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    if targets.is_empty() {
        return Err("请选择至少一个目标".into());
    }
    let mut s = hub.settings()?;
    let skill = s
        .library
        .iter()
        .find(|s| s.id == skill_id)
        .ok_or("管理版本不存在")?
        .clone();
    valid_name(&skill.name)?;
    let content = snapshot(&hub.library_path(&skill)?)?;
    validate_skill(&content)?;
    let mut changes = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for target in targets {
        let path = hub.target(&target, true)?.join(&skill.name);
        if !seen.insert(path.clone()) {
            continue;
        }
        ensure_no_symlink_ancestors(&path)?;
        changes.push(Change {
            path: path_string(&path),
            before: snapshot(&path)?,
            after: content.clone(),
            sensitive: false,
        });
        s.installations.insert(path_string(&path), skill_id.clone());
    }
    changes.push(hub.settings_change(&s)?);
    hub.queue(format!("分配 Skill · {}",skill.name),changes,vec!["Codex 与 Cursor 共用 .agents/skills；项目目录还可能被 Antigravity 读取。共享发现路径中的变更会影响所有读取它的 Agent，不能在此路径上分别停用。".into(),"采用普通文件复制；请检查原生目录中的其他同名资源。写入完成不代表 Agent 已加载，可能需要重启或新建会话。".into()])
}
#[tauri::command]
#[specta::specta]
pub async fn check_update(
    app: AppHandle,
    hub: State<'_, Hub>,
    skill_id: String,
) -> Result<PlanView> {
    let _operation = hub.operation();
    let skill = hub
        .settings()?
        .library
        .into_iter()
        .find(|s| s.id == skill_id)
        .ok_or("资源不存在")?;
    if skill.pinned {
        return Err("此 Skill 已固定版本，请先解除固定再检查更新".into());
    }
    hub.library_path(&skill)?;
    if !skill.source.starts_with("https://") {
        let path = access::normalize_choice(&checked_path(&skill.source)?, PathPurpose::Source)?;
        access::confirm(
            &app,
            "读取 Skill 更新来源",
            format!(
                "将重新读取以下本地来源目录并比较内容：\n{}\n\n不会执行其中脚本。",
                path_string(&path)
            ),
        )
        .await?;
        hub.grant(PathPurpose::Source, path)?;
    }
    let (content, revision, name, description) =
        get_source(&app, &hub, &skill.source, &skill.subdir).await?;
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let mut s = hub.settings()?;
    let current = s
        .library
        .iter_mut()
        .find(|s| s.id == skill_id)
        .ok_or("资源已被移除")?;
    if current.revision != skill.revision || current.pinned {
        return Err("资源状态已改变，请重新检查".into());
    }
    // Keep the deployment name stable even if upstream changes its frontmatter name.
    current.revision = revision;
    current.description = description;
    hub.queue(format!("更新管理版本 · {}",skill.name),vec![Change{path:skill.folder.clone(),before:snapshot(Path::new(&skill.folder))?,after:content,sensitive:false},hub.settings_change(&s)?],vec![format!("上游名称：{name}。此操作只更新 Agent Hub 管理的版本；请再分配给目标 Agent，目标上的外部修改仍需逐项预览。")])
}
#[tauri::command]
#[specta::specta]
pub fn pin_skill(hub: State<Hub>, skill_id: String, pinned: bool) -> Result<()> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let mut s = hub.settings()?;
    s.library
        .iter_mut()
        .find(|s| s.id == skill_id)
        .ok_or("资源不存在")?
        .pinned = pinned;
    hub.save(&s)
}
#[tauri::command]
#[specta::specta]
pub fn delete_skill(hub: State<Hub>, skill_id: String) -> Result<PlanView> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let mut s = hub.settings()?;
    let skill = s
        .library
        .iter()
        .find(|s| s.id == skill_id)
        .ok_or("资源不存在")?
        .clone();
    hub.library_path(&skill)?;
    let scan = scanner::run(&hub)?;
    let mut references: std::collections::BTreeSet<_> = scan
        .resources
        .iter()
        .filter(|r| r.library_id.as_deref() == Some(&skill_id))
        .map(|r| r.path.clone())
        .collect();
    references.extend(
        s.installations
            .iter()
            .filter(|(p, id)| *id == &skill_id && Path::new(p).exists())
            .map(|(p, _)| p.clone()),
    );
    if !references.is_empty() {
        return Err(format!(
            "仍发现 {} 个关联或内容一致的安装位置，请先移除后再处理：\n{}",
            references.len(),
            references.into_iter().collect::<Vec<_>>().join("\n")
        ));
    }
    s.library.retain(|s| s.id != skill_id);
    s.installations.retain(|_, id| id != &skill_id);
    hub.queue(
        format!("删除管理版本 · {}", skill.name),
        vec![
            Change {
                path: skill.folder.clone(),
                before: snapshot(Path::new(&skill.folder))?,
                after: Snapshot {
                    exists: false,
                    directory: false,
                    entries: vec![],
                    directories: vec![],
                },
                sensitive: false,
            },
            hub.settings_change(&s)?,
        ],
        vec!["仅删除 由 Agent Hub 管理的版本，不删除原始来源。".into()],
    )
}
#[tauri::command]
#[specta::specta]
pub async fn reveal_resource(
    app: AppHandle,
    hub: State<'_, Hub>,
    resource_id: String,
) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let scan = scanner::run(&hub)?;
    let resource = scan
        .resources
        .iter()
        .find(|r| r.id == resource_id)
        .ok_or("资源已不存在，请重新扫描")?;
    let path = Path::new(&resource.canonical);
    if !path.is_dir() && !path.is_file() {
        return Err("扫描到的文件或目录当前不可用".into());
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|_| "无法用系统默认应用打开此文件或目录".into())
}

#[tauri::command]
#[specta::specta]
pub fn remove_resource(hub: State<Hub>, resource_id: String) -> Result<PlanView> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let scan = scanner::run(&hub)?;
    let r = scan
        .resources
        .iter()
        .find(|r| r.id == resource_id)
        .ok_or("资源已改变，请重新扫描")?;
    if r.kind == "plugin" {
        return Err("插件请通过所属 Agent 的原生管理入口卸载".into());
    }
    if r.kind == "mcp" {
        if let Some(project) = r.scope.strip_suffix(" (Claude local)") {
            let path = Path::new(&r.path);
            ensure_no_symlink_ancestors(path)?;
            let before = snapshot(path)?;
            let mut value: Value = serde_json::from_str(&text_of(&before)?)
                .map_err(|_| "现有配置不是标准 JSON，已停止")?;
            let servers = value
                .get_mut("projects")
                .and_then(|v| v.get_mut(project))
                .and_then(|v| v.get_mut("mcpServers"))
                .and_then(Value::as_object_mut)
                .ok_or("项目 local 配置已改变")?;
            servers.remove(&r.name);
            return hub.queue(format!("移除项目 local MCP · {}",r.name),vec![Change{path:r.path.clone(),before,after:file_snapshot(serde_json::to_string_pretty(&value).map_err(|_|"配置序列化失败")?+"\n"),sensitive:true}],vec!["只移除 Claude 指定项目的 local 项，其他项目和 user 配置保留。需要在原生会话中确认重载。".into()]);
        }
        return mcp_plan(
            &hub,
            &r.name,
            None,
            vec![Target {
                agent: r.agents[0].clone(),
                project: if r.scope == "global" {
                    None
                } else {
                    Some(r.scope.clone())
                },
            }],
        );
    }
    let path = Path::new(&r.path);
    ensure_no_symlink_ancestors(path)?;
    let mut s = hub.settings()?;
    let affected: Vec<_> = scan
        .resources
        .iter()
        .filter(|other| {
            other.kind == "skill" && Path::new(&other.canonical).starts_with(&r.canonical)
        })
        .collect();
    let agents: std::collections::BTreeSet<_> = affected
        .iter()
        .flat_map(|other| other.agents.iter().map(|a| scanner::display_name(a)))
        .collect();
    let mut notes = vec![format!(
        "将删除此真实目录及其内容：{}。根据当前扫描，会影响 {}。关联链接将失效，未扫描的项目或入口不在此清单内。",
        r.path, agents.into_iter().collect::<Vec<_>>().join("、")
    )];
    notes.extend(affected.iter().map(|other| {
        format!(
            "受影响入口：{} → {}（{}）",
            other.path,
            other.canonical,
            other
                .agents
                .iter()
                .map(|a| scanner::display_name(a))
                .collect::<Vec<_>>()
                .join("、")
        )
    }));
    let managed = r
        .library_id
        .as_ref()
        .and_then(|id| s.library.iter().find(|skill| &skill.id == id))
        .and_then(|skill| hub.library_path(skill).ok())
        .filter(|folder| {
            source_snapshot(folder).is_ok_and(|content| validate_skill(&content).is_ok())
        });
    notes.push(match managed {
        Some(folder) => format!(
            "Agent Hub 管理副本保留：{}。它与待删除目录独立，不会自动同步。",
            path_string(&folder)
        ),
        None => {
            "未确认到独立的 Agent Hub 管理副本。应用前会保存操作备份，可通过变更历史撤销。".into()
        }
    });
    s.installations
        .retain(|entry, _| !Path::new(entry).starts_with(path));
    hub.queue(
        format!("移除安装 · {}", r.name),
        vec![
            Change {
                path: r.path.clone(),
                before: snapshot(path)?,
                after: Snapshot {
                    exists: false,
                    directory: false,
                    entries: vec![],
                    directories: vec![],
                },
                sensitive: false,
            },
            hub.settings_change(&s)?,
        ],
        notes,
    )
}
#[tauri::command]
#[specta::specta]
pub fn preview_mcp(
    hub: State<Hub>,
    name: String,
    config: String,
    targets: Vec<Target>,
) -> Result<PlanView> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let value: Value = serde_json::from_str(&config).map_err(|_| "请输入标准 JSON 配置对象")?;
    if !value.is_object() {
        return Err("配置必须是 JSON 对象".into());
    }
    // Values stay backend-only. Existing credentials in other entries are preserved, not copied between agents.
    mcp_plan(&hub, &name, Some(value), targets)
}
pub(crate) fn mcp_plan(
    hub: &Hub,
    name: &str,
    config: Option<Value>,
    targets: Vec<Target>,
) -> Result<PlanView> {
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err("MCP 名称不能为空、含控制字符或超过 256 字节".into());
    }
    if targets.is_empty() {
        return Err("请选择目标 Agent".into());
    }
    let mut changes = vec![];
    for t in targets {
        let path = hub.target(&t, false)?;
        ensure_no_symlink_ancestors(&path)?;
        let before = snapshot(&path)?;
        let text = text_of(&before)?;
        let after = if t.agent == "codex" {
            let mut doc = text
                .parse::<toml_edit::DocumentMut>()
                .map_err(|_| "现有 TOML 无效，已停止")?;
            if doc.get("mcp_servers").is_none() {
                doc["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            let table = doc["mcp_servers"]
                .as_table_like_mut()
                .ok_or("mcp_servers 不是表")?;
            if let Some(v) = &config {
                let adapted = adapt_config(v, &t.agent)?;
                let rendered = toml_edit::ser::to_string(&json!({"server":adapted}))
                    .map_err(|_| "配置无法转为 TOML")?;
                let parsed = rendered
                    .parse::<toml_edit::DocumentMut>()
                    .map_err(|_| "TOML 序列化错误")?;
                if let Some(existing) = table.get_mut(name) {
                    let existing = existing.as_table_like_mut().ok_or("现有 MCP 项不是表")?;
                    if existing.contains_key("command") != v.get("command").is_some() {
                        return Err(
                            "更换传输方式需在原生配置中处理，避免保留错误的凭据和扩展字段".into(),
                        );
                    }
                    for (key, item) in parsed["server"].as_table_like().ok_or("配置不是表")?.iter()
                    {
                        if key != "env" && key != "http_headers" {
                            existing.insert(key, item.clone());
                        }
                    }
                } else {
                    table.insert(name, parsed["server"].clone());
                }
            } else {
                table.remove(name);
            }
            doc.to_string()
        } else {
            let mut value: Value = if text.is_empty() {
                json!({})
            } else {
                serde_json::from_str(&text)
                    .map_err(|_| "现有配置不是标准 JSON；为保留注释与内容，已停止写入")?
            };
            let obj = value.as_object_mut().ok_or("配置根必须是对象")?;
            let servers = obj
                .entry("mcpServers")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("mcpServers 不是对象")?;
            if let Some(v) = &config {
                let adapted = adapt_config(v, &t.agent)?;
                if let Some(existing) = servers.get_mut(name) {
                    let existing = existing.as_object_mut().ok_or("现有 MCP 项不是对象")?;
                    if existing.contains_key("command") != v.get("command").is_some() {
                        return Err(
                            "更换传输方式需在原生配置中处理，避免保留错误的凭据和扩展字段".into(),
                        );
                    }
                    for (key, value) in adapted.as_object().ok_or("配置不是对象")? {
                        if key != "env" && key != "headers" {
                            existing.insert(key.clone(), value.clone());
                        }
                    }
                } else {
                    servers.insert(name.into(), adapted);
                }
            } else {
                servers.remove(name);
            }
            serde_json::to_string_pretty(&value).map_err(|_| "配置序列化失败")? + "\n"
        };
        if changes
            .iter()
            .any(|c: &Change| c.path == path_string(&path))
        {
            return Err("目标文件重复，请分次操作".into());
        }
        changes.push(Change {
            path: path_string(&path),
            before,
            after: file_snapshot(after),
            sensitive: true,
        });
    }
    hub.queue(format!("{} MCP · {name}",if config.is_some(){"配置"}else{"移除"}),changes,vec!["预览隐藏所有字符串值，字段结构可见。备份包含原始配置，保存于仅当前用户可访问的应用目录。".into(),"配置写入后，需在各 Agent 原生界面重载并完成 OAuth。此操作不会启动 MCP、迁移密钥或复制已有凭据。".into()])
}
fn adapt_config(v: &Value, agent: &str) -> Result<Value> {
    let mut v = v.clone();
    let obj = v.as_object_mut().ok_or("配置必须是对象")?;
    // The editor accepts only a portable subset. Advanced native fields must be edited natively.
    for key in obj.keys() {
        if !["command", "args", "env", "url", "headers"].contains(&key.as_str()) {
            return Err(format!("暂不支持跨端字段 {key}，请在原生配置中管理"));
        }
    }
    if let Some(env) = obj.get("env") {
        if !env.is_object() || env.as_object().unwrap().values().any(|v| !v.is_string()) {
            return Err("env 必须是字符串映射".into());
        }
    }
    if let Some(headers) = obj.get("headers") {
        if !headers.is_object()
            || headers
                .as_object()
                .unwrap()
                .values()
                .any(|v| !v.is_string())
        {
            return Err("headers 必须是字符串映射".into());
        }
    }
    // Secret delivery is deliberately excluded from the first release. Accept placeholders, never literal env/header values.
    for key in ["env", "headers"] {
        if let Some(m) = obj.get(key).and_then(Value::as_object) {
            if !m.is_empty() {
                return Err("第一版仅管理无凭据 MCP 配置。请保留原生凭据配置，或添加后在所属 Agent 中登录 OAuth。".into());
            }
        }
    }
    let stdio = obj
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    let remote = obj
        .get("url")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    if stdio == remote {
        return Err("必须提供 command 或 url，且只能选择一种传输方式".into());
    }
    if let Some(args) = obj.get("args") {
        if !args.is_array() || args.as_array().unwrap().iter().any(|a| !a.is_string()) {
            return Err("args 必须是字符串数组".into());
        }
    }
    if remote {
        let url = reqwest::Url::parse(obj["url"].as_str().unwrap()).map_err(|_| "URL 无效")?;
        access::allow_remote_url(&url)?;
        if !["https", "http"].contains(&url.scheme())
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
        {
            return Err("URL 只接受 HTTP(S)，不能包含凭据或查询参数".into());
        }
        if agent == "claude" {
            obj.insert("type".into(), json!("http"));
        }
        if agent == "antigravity" {
            let url = obj.remove("url").unwrap();
            obj.insert("serverUrl".into(), url);
        }
        if agent == "codex" {
            if let Some(h) = obj.remove("headers") {
                obj.insert("http_headers".into(), h);
            }
        }
    }
    Ok(v)
}
#[tauri::command]
#[specta::specta]
pub async fn apply_plan(app: AppHandle, hub: State<'_, Hub>, plan_id: String) -> Result<History> {
    let _operation = hub.operation();
    let description = {
        let plans = hub.plans.lock().map_err(|_| "预览锁不可用")?;
        let plan = plans.get(&plan_id).ok_or("预览已失效，请重新生成")?;
        format!(
            "{}\n\n将修改以下 {} 个目标并保存备份：\n{}\n\n是否应用？",
            plan.title,
            plan.changes.len(),
            plan.changes
                .iter()
                .map(|c| c.path.clone())
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    access::confirm(&app, "确认应用本机文件变更", description).await?;
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    hub.apply(&plan_id)
}
#[tauri::command]
#[specta::specta]
pub fn history(hub: State<Hub>) -> Result<Vec<History>> {
    let home = hub.home()?;
    let globals: Vec<_> = scanner::AGENTS
        .iter()
        .filter_map(|agent| {
            hub.target(
                &Target {
                    agent: (*agent).into(),
                    project: None,
                },
                false,
            )
            .ok()
        })
        .collect();
    let global_skills = scanner::skill_roots(&home, None);
    let mut records = hub.records()?;
    for record in &mut records {
        let mut projects = std::collections::BTreeSet::new();
        for path in &record.paths {
            if globals.iter().any(|p| p == Path::new(path))
                || global_skills
                    .iter()
                    .any(|(root, _)| Path::new(path).starts_with(root))
            {
                continue;
            }
            let mut candidate: Option<&str> = None;
            for marker in [
                "/.agents/",
                "/.claude/",
                "/.cursor/",
                "/.codex/",
                "/.gemini/",
            ] {
                if let Some((base, _)) = path.rsplit_once(marker) {
                    if Path::new(base) != home && !base.is_empty() {
                        if candidate.is_none_or(|old| base.len() > old.len()) {
                            candidate = Some(base);
                        }
                    }
                }
            }
            if let Some(base) = candidate {
                projects.insert(base.to_string());
            }
        }
        record.required_projects = projects.into_iter().collect();
    }
    Ok(records)
}
#[tauri::command]
#[specta::specta]
pub fn preview_undo(hub: State<Hub>, history_id: String) -> Result<PlanView> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    valid_name(&history_id)?;
    let record = hub
        .records()?
        .into_iter()
        .find(|r| r.id == history_id)
        .ok_or("记录不存在")?;
    if record.undone_by.is_some() {
        return Err("此操作已经撤销或恢复，请刷新历史；需要重做时请选择对应的反向操作记录".into());
    }
    let recovery = record.status != "applied";
    let plan: Plan = serde_json::from_slice(
        &fs::read(
            hub.root()?
                .join("backups")
                .join(format!("{history_id}.json")),
        )
        .map_err(|_| "无法读取备份")?,
    )
    .map_err(|_| "备份格式无效")?;
    let mut changes = vec![];
    let mut notes = vec![if recovery {
        "失败恢复会逐文件还原操作前备份；下方列明已恢复、已写入或存在后续修改的目标，请核对差异。"
            .into()
    } else {
        "恢复此操作之前的内容；应用前会再次检查外部修改。".into()
    }];
    for c in plan.changes {
        hub.authorize_change(&c).map_err(|e| format!("无法撤销此目标：{}。若项目已停止扫描，请先在设置与备份中重新添加原项目，再生成预览。{e}",c.path))?;
        let current = snapshot(Path::new(&c.path))?;
        if recovery {
            notes.push(format!(
                "{}：{}",
                c.path,
                if current == c.before {
                    "已是操作前内容，无需恢复"
                } else if current == c.after {
                    "仍为该操作写入的内容，将恢复"
                } else {
                    "存在后续修改或部分写入，应用将覆盖当前内容；请逐项核对"
                }
            ));
        }
        if !recovery && current != c.after {
            return Err(format!(
                "文件存在后续变更，为避免覆盖不能直接撤销：{}。请重新分配或从备份手工合并。",
                c.path
            ));
        }
        changes.push(Change {
            path: c.path,
            before: current,
            after: c.before,
            sensitive: c.sensitive,
        });
    }
    let view = hub.queue(
        format!(
            "{} · {}",
            if recovery {
                "恢复失败操作"
            } else {
                "撤销"
            },
            plan.title
        ),
        changes,
        notes,
    )?;
    hub.plans
        .lock()
        .map_err(|_| "预览锁不可用")?
        .get_mut(&view.id)
        .ok_or("预览已失效")?
        .undo_of = Some(history_id);
    Ok(view)
}
#[tauri::command]
#[specta::specta]
pub fn export_library(hub: State<Hub>, path: String) -> Result<String> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let p = checked_path(&path)?;
    hub.require_grant(PathPurpose::Export, &p)?;
    ensure_no_symlink_ancestors(&p)?;
    if p.exists() {
        return Err("导出路径已存在，请换一个文件名，避免覆盖".into());
    }
    let s = hub.settings()?;
    let mut skills = vec![];
    for skill in s.library {
        let content = snapshot(&hub.library_path(&skill)?)?;
        validate_skill(&content)?;
        skills.push(BundleSkill { skill, content });
    }
    let scan = scanner::run(&hub)?;
    let assignments = scan
        .resources
        .iter()
        .filter_map(|r| r.library_id.as_ref().map(|sid| (sid, r)))
        .flat_map(|(sid, r)| {
            r.agents.iter().map(move |a| BundleAssignment {
                skill_id: sid.clone(),
                target: Target {
                    agent: a.clone(),
                    project: if r.scope == "global" {
                        None
                    } else {
                        Some(r.scope.clone())
                    },
                },
            })
        })
        .collect();
    let data = serde_json::to_vec(&Bundle {
        format: 1,
        skills,
        assignments,
    })
    .map_err(|_| "导出失败")?;
    if data.len() > 100 * 1024 * 1024 {
        return Err("导出超过 100 MB，请减少资源数量".into());
    }
    write_new(&p, &data)?;
    Ok(path_string(&p))
}
#[tauri::command]
#[specta::specta]
pub fn import_library(hub: State<Hub>, path: String, overwrite: bool) -> Result<PlanView> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    let p = checked_path(&path)?;
    hub.require_grant(PathPurpose::Import, &p)?;
    if fs::metadata(&p).map_err(|_| "导入文件不存在")?.len() > 100 * 1024 * 1024 {
        return Err("导入包超过 100 MB".into());
    }
    let bundle: Bundle = serde_json::from_slice(&fs::read(p).map_err(|_| "读取导入包失败")?)
        .map_err(|_| "导入包格式无效")?;
    if bundle.format != 1 {
        return Err("不支持的导入包版本".into());
    }
    let mut s = hub.settings()?;
    let mut changes = vec![];
    let mut notes=vec!["只恢复 Agent Hub 管理数据；分配关系作为下方参考，请核对本机项目路径后再分配。导入包不含 MCP 配置或凭据。".into()];
    let mut seen = std::collections::BTreeSet::new();
    let (mut added, mut replaced, mut skipped) = (0, 0, 0);
    for item in bundle.skills {
        valid_name(&item.skill.name)?;
        validate_skill(&item.content)?;
        let mut skill = item.skill;
        skill.id = hash(format!("{}#{}", skill.source, skill.subdir).as_bytes());
        if !seen.insert(skill.id.clone()) {
            return Err("导入包含重复来源".into());
        }
        skill.folder = path_string(&hub.root()?.join("library").join(&skill.id));
        if let Some(existing) = s.library.iter().find(|x| x.id == skill.id) {
            if !overwrite {
                skipped += 1;
                notes.push(format!(
                    "跳过同来源资源：{}；保留本机版本及固定状态",
                    existing.name
                ));
                continue;
            }
            replaced += 1;
            notes.push(format!(
                "覆盖同来源资源：{}；版本 {} → {}，固定状态 {} → {}",
                skill.name, existing.revision, skill.revision, existing.pinned, skill.pinned
            ));
        } else {
            added += 1;
            notes.push(format!("新增管理副本：{}", skill.name));
        }
        changes.push(Change {
            path: skill.folder.clone(),
            before: snapshot(Path::new(&skill.folder))?,
            after: item.content,
            sensitive: false,
        });
        s.library.retain(|x| x.id != skill.id);
        s.library.push(skill);
    }
    for a in bundle.assignments {
        notes.push(format!(
            "原分配：{} → {} / {}",
            a.skill_id.chars().take(10).collect::<String>(),
            a.target.agent,
            a.target.project.unwrap_or("全局".into())
        ));
    }
    changes.push(hub.settings_change(&s)?);
    notes.insert(0,format!("合并导入：新增 {added} 项，覆盖 {replaced} 项，跳过 {skipped} 项；其他本机资源保持不变。不恢复项目列表、操作历史或原生 Agent 配置。"));
    if added == 0 && replaced == 0 {
        return Err(format!("无需导入：已跳过 {skipped} 个同来源资源，本机内容保持不变。如需覆盖，请在导入窗口明确选择覆盖策略。"));
    }
    hub.queue("导入Agent Hub 管理数据备份".into(), changes, notes)
}
#[tauri::command]
#[specta::specta]
pub fn plugin_guide(agent: String) -> Result<()> {
    if !scanner::AGENTS.contains(&agent.as_str()) {
        return Err("未知 Agent".into());
    }
    std::process::Command::new("/usr/bin/open")
        .arg(scanner::guide(&agent))
        .spawn()
        .map_err(|_| "无法打开官方管理文档")?;
    Ok(())
}
