use crate::{app::Hub, files::*, model::*, scan};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tauri::State;

#[derive(Clone, serde::Serialize, specta::Type)]
pub struct NativeControl {
    pub resource_id: String,
    pub agent: String,
    pub enabled: Option<bool>,
    pub unavailable_label: String,
    pub reason: String,
    pub path: String,
    pub selector: String,
    pub context: Option<String>,
}
type Cache = BTreeMap<PathBuf, (Snapshot, Value)>;

fn read<'a>(cache: &'a mut Cache, path: &Path) -> Result<&'a Value> {
    if !cache.contains_key(path) {
        let before = snapshot(path)?;
        let text = text_of(&before)?;
        let value = if text.trim().is_empty() {
            json!({})
        } else if path.extension().is_some_and(|e| e == "toml") {
            toml_edit::de::from_str::<Value>(&text).map_err(|_| "TOML 配置无法解析")?
        } else {
            serde_json::from_str::<Value>(&text).map_err(|_| "JSON 配置无法解析")?
        };
        if !value.is_object() {
            return Err("原生配置必须是对象".into());
        }
        cache.insert(path.to_path_buf(), (before, value));
    }
    Ok(&cache.get(path).unwrap().1)
}

fn inspect(
    hub: &Hub,
    r: &Resource,
    agent: &str,
    project: Option<&str>,
    cache: &mut Cache,
) -> Result<NativeControl> {
    let mut c = NativeControl {
        resource_id: r.id.clone(),
        agent: agent.into(),
        enabled: None,
        unavailable_label: "尚未接入".into(),
        reason: "原生启停接口尚未接入，请在所属 Agent 中管理".into(),
        path: String::new(),
        selector: String::new(),
        context: None,
    };
    let home = hub.home()?;
    let source_project = if r.scope == "global" {
        None
    } else {
        Some(r.scope.trim_end_matches(" (Claude local)"))
    };
    let base = source_project
        .map(PathBuf::from)
        .unwrap_or_else(|| home.clone());
    if agent == "cursor" {
        c.reason = if r.kind == "mcp" {
            "Cursor 将 MCP 启停状态存入内部存储；本机版本同时存在项目级和配置档级状态，并由功能开关选择。mcp.json 中存在配置不等于已启用，请在 Cursor Customize 中切换"
        } else {
            "尚未接入 Cursor 此类资源的原生启停状态；扫描文件或 manifest 不能判定是否启用，请在 Cursor Customize 中管理"
        }.into();
        c.unavailable_label = "需在 Cursor 管理".into();
    } else if agent == "antigravity" && r.kind == "plugin" {
        c.reason = "插件目录可能是缓存；Antigravity CLI 可通过 agy plugin list --json 查询、plugin enable/disable 管理，当前尚未接入该协议，不能把 manifest 当成启用记录".into();
        c.unavailable_label = "需原生插件管理".into();
    }
    if r.kind == "skill" && agent == "claude" {
        if Path::new(&r.path).parent() != Some(base.join(".claude/skills").as_path()) {
            c.unavailable_label = "需确认来源".into();
            c.reason = "插件、嵌套或特殊来源 Skill 需通过对应原生入口管理".into();
            return Ok(c);
        }
        c.path = path_string(&base.join(".claude/settings.json"));
        c.selector = Path::new(&r.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if c.selector != r.name {
            c.unavailable_label = "需确认调用名".into();
            c.reason = "目录名与 Skill 名称不同，需在 Claude /skills 中确认实际调用名后管理".into();
            return Ok(c);
        }
        if ["synced", "anthropic-skills"].contains(&c.selector.as_str()) {
            return Ok(c);
        }
        // Edit the existing local override when present, rather than writing a shadowed project value.
        if source_project.is_some() {
            let local_path = base.join(".claude/settings.local.json");
            let local = read(cache, &local_path)?;
            if local.get("skillOverrides").is_some_and(|v| !v.is_object()) {
                return Err("local skillOverrides 格式无效".into());
            }
            if local
                .get("skillOverrides")
                .and_then(|v| v.get(&c.selector))
                .is_some()
            {
                c.path = path_string(&local_path);
            }
        }
        let value = read(cache, Path::new(&c.path))?;
        if value.get("skillOverrides").is_some_and(|v| !v.is_object()) {
            return Err("skillOverrides 格式无效".into());
        }
        if value
            .get("skillOverrides")
            .and_then(|v| v.get(&c.selector))
            .is_some_and(|v| !v.is_string())
        {
            return Err("Skill 可见性设置无效".into());
        }
        c.enabled = match value
            .get("skillOverrides")
            .and_then(|v| v.get(&c.selector))
            .and_then(Value::as_str)
        {
            None | Some("on") => Some(true),
            Some("off") => Some(false),
            _ => {
                c.unavailable_label = "部分可见".into();
                c.reason = "原生配置为部分可见状态，请在 Claude /skills 中管理".into();
                return Ok(c);
            }
        };
        c.reason = "按名称修改此作用域 skillOverrides；存在项目 local 覆盖时修改该覆盖文件。同名 Skill 可能一并受影响；保留文件，组织策略与会话加载需另行确认".into();
    } else if r.kind == "skill" && agent == "codex" {
        if r.path != r.canonical {
            c.unavailable_label = "需核验路径映射".into();
            c.reason = "此处是链接或映射入口，路径选择器匹配尚未核验；请在实际目录条目或 Codex 原生 /skills 中管理".into();
            return Ok(c);
        }
        c.path = path_string(&if source_project.is_some() {
            base.join(".codex/config.toml")
        } else {
            scan::codex_home(&home).join("config.toml")
        });
        c.selector = path_string(&Path::new(&r.path).join("SKILL.md"));
        let value = read(cache, Path::new(&c.path))?;
        if value.get("skills").is_some_and(|v| !v.is_object()) {
            return Err("skills 配置格式无效".into());
        }
        let configs = value.get("skills").and_then(|v| v.get("config"));
        if configs.is_some_and(|v| !v.is_array()) {
            return Err("skills.config 格式无效".into());
        }
        let mut enabled = true;
        if let Some(items) = configs.and_then(Value::as_array) {
            for item in items {
                let selectors = usize::from(item.get("path").is_some())
                    + usize::from(item.get("name").is_some());
                if !item.is_object() || selectors != 1 {
                    return Err("skills.config 中存在无效或重复选择器".into());
                }
                if item.get("path").and_then(Value::as_str) == Some(c.selector.as_str())
                    || item.get("name").and_then(Value::as_str) == Some(r.name.as_str())
                {
                    enabled = item
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .ok_or("匹配的 Skill 启用设置无效")?;
                }
            }
        }
        c.enabled = Some(enabled);
        c.reason = "按 SKILL.md 路径设置 skills.config；保留源文件，需重启 Codex，项目配置还受信任状态影响".into();
    } else if r.kind == "mcp" && (agent == "codex" || agent == "antigravity") {
        // Change the scanned entry itself; never reconstruct the server or discard credentials.
        c.path = r.path.clone();
        c.selector = r.name.clone();
        let value = read(cache, Path::new(&c.path))?;
        let server = scan::servers(value, agent)
            .and_then(|servers| servers.get(&c.selector))
            .filter(|v| v.is_object())
            .ok_or("扫描到的 MCP 配置已不存在或格式无效")?;
        let field = if agent == "codex" {
            "enabled"
        } else {
            "disabled"
        };
        let flag = match server.get(field) {
            Some(v) => v.as_bool().ok_or("MCP 启停字段必须是布尔值")?,
            None => agent == "codex",
        };
        c.enabled = Some(if agent == "codex" { flag } else { !flag });
        c.reason = format!(
            "仅修改此文件中同名服务的 {field} 字段，保留其他字段与凭据；这是此配置入口的状态，更高层覆盖、组织策略及当前会话重载需在原生 Agent 中确认"
        );
    } else if r.kind == "mcp" && agent == "claude" {
        let context = project.or(source_project);
        let Some(context) = context else {
            c.unavailable_label = "需选择项目".into();
            c.reason = "Claude MCP 原生停用按项目保存，请先在抽屉中选择已添加的项目".into();
            return Ok(c);
        };
        if source_project.is_some_and(|p| p != context) {
            c.unavailable_label = "不属于当前项目".into();
            c.reason = "该配置属于其他项目，不适用于当前项目上下文".into();
            return Ok(c);
        }
        c.context = Some(context.into());
        c.path = path_string(&home.join(".claude.json"));
        c.selector = r.name.clone();
        let value = read(cache, Path::new(&c.path))?;
        if value.get("projects").is_some_and(|v| !v.is_object()) {
            return Err("Claude projects 配置格式无效".into());
        }
        let entry = value.get("projects").and_then(|v| v.get(context));
        if entry.is_some_and(|v| !v.is_object()) {
            return Err("Claude 项目记录格式无效".into());
        }
        let disabled = entry.and_then(|v| v.get("disabledMcpServers"));
        if disabled
            .is_some_and(|v| !v.is_array() || v.as_array().unwrap().iter().any(|x| !x.is_string()))
        {
            return Err("disabledMcpServers 格式无效".into());
        }
        c.enabled = Some(
            !disabled
                .and_then(Value::as_array)
                .is_some_and(|v| v.iter().any(|x| x.as_str() == Some(r.name.as_str()))),
        );
        c.reason = format!(
            "仅对项目 {} 修改 disabledMcpServers，作用于此项目的同名服务（包含其他来源同名项）；不删除配置，不代表全局停用",
            context
        );
    } else if r.kind == "plugin" && agent == "claude" {
        c.path = path_string(&base.join(".claude/settings.json"));
        let value = read(cache, Path::new(&c.path))?;
        if value.get("enabledPlugins").is_some_and(|v| !v.is_object()) {
            return Err("enabledPlugins 格式无效".into());
        }
        let mut records: BTreeMap<String, (Value, String)> = value
            .get("enabledPlugins")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|m| m.iter())
            .map(|(k, v)| (k.clone(), (v.clone(), c.path.clone())))
            .collect();
        if source_project.is_some() {
            let local_path = base.join(".claude/settings.local.json");
            let local = read(cache, &local_path)?;
            if local.get("enabledPlugins").is_some_and(|v| !v.is_object()) {
                return Err("local enabledPlugins 格式无效".into());
            }
            for (k, v) in local
                .get("enabledPlugins")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|m| m.iter())
            {
                records.insert(k.clone(), (v.clone(), path_string(&local_path)));
            }
        }
        let candidates: Vec<_> = records
            .iter()
            .filter(|(k, _)| k.as_str() == r.name || k.starts_with(&format!("{}@", r.name)))
            .collect();
        if candidates.len() != 1 {
            c.unavailable_label = if candidates.is_empty() {
                "缺少启用记录"
            } else {
                "启用记录不唯一"
            }
            .into();
            c.reason =
                "未找到唯一的原生 enabledPlugins 记录；manifest 可能只是缓存，不能判定启用状态"
                    .into();
            return Ok(c);
        }
        c.selector = candidates[0].0.clone();
        c.enabled = candidates[0].1 .0.as_bool();
        c.path = candidates[0].1 .1.clone();
        if c.enabled.is_none() {
            return Err("插件启用记录格式无效".into());
        }
        c.reason = "修改已有 enabledPlugins 记录；存在项目 local 覆盖时修改该覆盖文件。停用将连带影响插件提供的 Skills、MCP 和 hooks，文件仍保留".into();
    }
    Ok(c)
}

fn validate_context(hub: &Hub, context: Option<&str>) -> Result<()> {
    if let Some(p) = context {
        if !hub.settings()?.projects.iter().any(|x| x == p) {
            return Err("请选择已添加的项目".into());
        }
        ensure_no_symlink_ancestors(Path::new(p))?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn toggle_states(
    hub: State<Hub>,
    project_context: Option<String>,
) -> Result<Vec<NativeControl>> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    validate_context(&hub, project_context.as_deref())?;
    let inventory = scan::run(&hub)?;
    let mut cache = Cache::new();
    let mut controls = Vec::new();
    for r in &inventory.resources {
        for agent in &r.agents {
            controls.push(
                inspect(&hub, r, agent, project_context.as_deref(), &mut cache).unwrap_or_else(
                    |reason| NativeControl {
                        resource_id: r.id.clone(),
                        agent: agent.clone(),
                        enabled: None,
                        unavailable_label: "配置读取异常".into(),
                        reason,
                        path: String::new(),
                        selector: String::new(),
                        context: None,
                    },
                ),
            );
        }
    }
    Ok(controls)
}

#[tauri::command]
#[specta::specta]
pub fn preview_toggle(
    hub: State<Hub>,
    resource_id: String,
    agent: String,
    project_context: Option<String>,
    expected_enabled: bool,
    expected_selector: String,
    expected_path: String,
    enabled: bool,
) -> Result<PlanView> {
    let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
    validate_context(&hub, project_context.as_deref())?;
    let inventory = scan::run(&hub)?;
    let r = inventory
        .resources
        .iter()
        .find(|r| r.id == resource_id && r.agents.contains(&agent))
        .ok_or("资源已改变，请重新扫描")?;
    let mut cache = Cache::new();
    let c = inspect(&hub, r, &agent, project_context.as_deref(), &mut cache)?;
    if c.enabled != Some(expected_enabled)
        || c.selector != expected_selector
        || c.path != expected_path
    {
        return Err("启用状态已改变或无法可靠操作，请重新扫描".into());
    }
    let (before, mut value) = cache.remove(Path::new(&c.path)).ok_or("原生状态尚未确认")?;
    let after = if agent == "codex" && r.kind == "mcp" {
        let mut doc = text_of(&before)?
            .parse::<toml_edit::DocumentMut>()
            .map_err(|_| "TOML 无法解析")?;
        doc["mcp_servers"][&c.selector]["enabled"] = toml_edit::value(enabled);
        file_snapshot(doc.to_string())
    } else if agent == "antigravity" && r.kind == "mcp" {
        value["mcpServers"][&c.selector]["disabled"] = json!(!enabled);
        file_snapshot(serde_json::to_string_pretty(&value).map_err(|_| "无法保存原生配置")? + "\n")
    } else if agent == "codex" && r.kind == "skill" {
        let mut doc = text_of(&before)?
            .parse::<toml_edit::DocumentMut>()
            .map_err(|_| "TOML 无法解析")?;
        if doc.get("skills").is_none() {
            doc["skills"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        if doc["skills"].get("config").is_none() {
            doc["skills"]["config"] = if doc["skills"].is_inline_table() {
                toml_edit::value(toml_edit::Array::new())
            } else {
                toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new())
            };
        }
        if let Some(items) = doc["skills"]["config"].as_array_of_tables_mut() {
            let mut t = toml_edit::Table::new();
            t["path"] = toml_edit::value(&c.selector);
            t["enabled"] = toml_edit::value(enabled);
            items.push(t);
        } else if let Some(items) = doc["skills"]["config"].as_array_mut() {
            let mut t = toml_edit::InlineTable::new();
            t.insert("path", c.selector.clone().into());
            t.insert("enabled", enabled.into());
            items.push(t);
        } else {
            return Err("skills.config 结构不支持编辑".into());
        }
        file_snapshot(doc.to_string())
    } else {
        if r.kind == "mcp" {
            let p = c.context.as_ref().ok_or("缺少项目上下文")?;
            let mut list = value
                .get("projects")
                .and_then(|v| v.get(p))
                .and_then(|v| v.get("disabledMcpServers"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            list.retain(|x| x.as_str() != Some(&c.selector));
            if !enabled {
                list.push(json!(c.selector));
            }
            value["projects"][p]["disabledMcpServers"] = json!(list);
        } else if r.kind == "skill" {
            value["skillOverrides"][&c.selector] = json!(if enabled { "on" } else { "off" });
        } else {
            value["enabledPlugins"][&c.selector] = json!(enabled);
        }
        file_snapshot(serde_json::to_string_pretty(&value).map_err(|_| "无法保存原生配置")? + "\n")
    };
    hub.queue(format!("{} {} · {}", if enabled { "启用" } else { "停用" }, scan::display_name(&agent), r.name),
        vec![Change { path: c.path, before, after, sensitive: true }],
        vec![c.reason, "仅修改原生启用配置；不删除资源文件或凭据，不承诺当前会话立即重载。应用后请在原生 Agent 中确认；可在变更历史撤销。".into()])
}
