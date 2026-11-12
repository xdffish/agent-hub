use crate::{access, app::Hub, model::*, scan};
use serde_json::{json, Value};
use std::{collections::BTreeMap, process::Stdio, time::Duration};
use tauri::State;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

fn initialize() -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"agent-hub-check","version":"0.1.0"}}})
}
fn resolve(value: &str) -> Result<String> {
    let mut result = value.to_string();
    for _ in 0..20 {
        let Some(start) = result.find("${") else {
            return Ok(result);
        };
        let end = result[start..]
            .find('}')
            .ok_or("配置含无法解析的环境变量引用")?
            + start;
        let var = &result[start + 2..end];
        let var = var.strip_prefix("env:").unwrap_or(var);
        let (name, default) = var
            .split_once(":-")
            .map(|(n, d)| (n, Some(d)))
            .unwrap_or((var, None));
        let val = std::env::var(name)
            .ok()
            .or_else(|| default.map(str::to_string))
            .ok_or("当前应用进程缺少配置引用的环境变量，请在 Agent 中检查或设置启动环境")?;
        result.replace_range(start..=end, &val);
    }
    Err("环境变量引用层数过多".into())
}
#[tauri::command]
#[specta::specta]
pub async fn check_mcp(
    hub: State<'_, Hub>,
    resource_id: String,
    expected_fingerprint: String,
) -> Result<String> {
    let _operation = hub.operation();
    let config = {
        let _lock = hub.lock.lock().map_err(|_| "管理库忙")?;
        let inventory = scan::run(&hub)?;
        let r = inventory
            .resources
            .iter()
            .find(|r| r.id == resource_id && r.kind == "mcp")
            .ok_or("配置已改变，请重新扫描")?;
        let root = scan::parse_config(std::path::Path::new(&r.path), &r.agents[0])?;
        let value = if r.scope.ends_with(" (Claude local)") {
            let p = r.scope.trim_end_matches(" (Claude local)");
            root.get("projects")
                .and_then(|v| v.get(p))
                .cloned()
                .ok_or("项目配置不存在")?
        } else {
            root
        };
        scan::servers(&value, &r.agents[0])
            .and_then(|v| v.get(&r.name))
            .cloned()
            .ok_or("MCP 配置不存在")?
    };
    let fingerprint = crate::files::hash(&serde_json::to_vec(&config).map_err(|_| "无法读取配置")?);
    if fingerprint != expected_fingerprint {
        return Err("配置已发生变化，本次未执行检查；请重新扫描后确认".into());
    }
    tokio::time::timeout(Duration::from_secs(12), check(config))
        .await
        .map_err(|_| "连接检查超时（12 秒）；临时检查进程已终止".to_string())?
}
async fn check(config: Value) -> Result<String> {
    if config.get("http_headers_helper").is_some()
        || config.get("auth").is_some()
        || config.get("authProviderType").is_some()
    {
        return Err("此配置使用原生鉴权或 helper，请在所属 Agent 中完成连接检查；本工具不会读取 OAuth 存储或执行凭据 helper".into());
    }
    if let Some(command) = config.get("command").and_then(Value::as_str) {
        let cwd = config
            .get("cwd")
            .and_then(Value::as_str)
            .map(resolve)
            .transpose()?
            .map(std::path::PathBuf::from);
        let configured_path = config
            .get("env")
            .and_then(Value::as_object)
            .and_then(|map| {
                map.iter().find(|(k, _)| {
                    if cfg!(windows) {
                        k.eq_ignore_ascii_case("PATH")
                    } else {
                        k.as_str() == "PATH"
                    }
                })
            })
            .map(|(_, v)| resolve(v.as_str().ok_or("env 必须为字符串映射")?))
            .transpose()?;
        let search_path = configured_path
            .map(std::ffi::OsString::from)
            .or_else(|| std::env::var_os("PATH"));
        let mut cmd = tokio::process::Command::new(crate::platform::command_path(
            &resolve(command)?,
            cwd.as_deref(),
            search_path.as_deref(),
        ));
        limited_environment(&mut cmd);
        if let Some(args) = config.get("args").and_then(Value::as_array) {
            for arg in args {
                cmd.arg(resolve(arg.as_str().ok_or("args 必须为字符串数组")?)?);
            }
        }
        if let Some(env) = config.get("env").and_then(Value::as_object) {
            for (key, value) in env {
                cmd.env(key, resolve(value.as_str().ok_or("env 必须为字符串映射")?)?);
            }
        }
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let (mut child, _group) = crate::platform::spawn(&mut cmd)
            .map_err(|_| "无法启动或隔离 MCP 进程，请检查命令路径与运行环境")?;
        let mut input = child.stdin.take().ok_or("无法打开进程输入")?;
        input
            .write_all(format!("{}\n", initialize()).as_bytes())
            .await
            .map_err(|_| "发送初始化请求失败")?;
        let output = child.stdout.take().ok_or("无法读取进程输出")?;
        let mut lines = BufReader::new(output.take(1024 * 1024)).lines();
        for _ in 0..100 {
            let Some(line) = lines.next_line().await.map_err(|_| "读取协议响应失败")?
            else {
                return Err("进程未返回有效的 MCP 初始化响应".into());
            };
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if value.get("id") == Some(&json!(1)) {
                    let message = validated(&value)?;
                    let _ = input
                        .write_all(
                            b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
                        )
                        .await;
                    let _ = child.kill().await;
                    return Ok(format!(
                        "{message}。临时 stdio 检查进程已关闭；未调用业务工具。"
                    ));
                }
            }
        }
        Err("未收到有效 MCP 响应；服务器输出已隐藏".into())
    } else {
        let url = config
            .get("url")
            .or_else(|| config.get("serverUrl"))
            .and_then(Value::as_str)
            .ok_or("没有 command 或 HTTP URL")?;
        let url = resolve(url)?;
        let parsed = reqwest::Url::parse(&url).map_err(|_| "URL 无效")?;
        access::allow_remote_url(&parsed)?;
        if !["http", "https"].contains(&parsed.scheme()) {
            return Err("只支持 HTTP(S) MCP 检查".into());
        }
        if config.get("type").and_then(Value::as_str) == Some("sse") {
            return Err("暂不检查旧 SSE 传输，请使用原生 Agent 检查".into());
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| "HTTP 客户端初始化失败")?;
        let mut headers = BTreeMap::new();
        if let Some(map) = config
            .get("headers")
            .or_else(|| config.get("http_headers"))
            .and_then(Value::as_object)
        {
            for (k, v) in map {
                headers.insert(k.clone(), resolve(v.as_str().ok_or("header 值无效")?)?);
            }
        }
        if let Some(map) = config.get("env_http_headers").and_then(Value::as_object) {
            for (k, v) in map {
                headers.insert(
                    k.clone(),
                    std::env::var(v.as_str().ok_or("环境 header 引用无效")?)
                        .map_err(|_| "缺少 header 所需环境变量")?,
                );
            }
        }
        if let Some(name) = config.get("bearer_token_env_var").and_then(Value::as_str) {
            headers.insert(
                "Authorization".into(),
                format!(
                    "Bearer {}",
                    std::env::var(name).map_err(|_| "缺少 bearer token 环境变量")?
                ),
            );
        }
        let mut request = client
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .json(&initialize());
        for (k, v) in &headers {
            request = request.header(k, v);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| "HTTP 连接失败，请检查网络和地址；详细错误已隐藏以保护 URL 和凭据")?;
        let status = response.status();
        if !status.is_success() {
            return Err(if status.as_u16() == 401 || status.as_u16() == 403 {
                "服务器要求认证，请在所属 Agent 原生入口完成登录".into()
            } else {
                format!(
                    "HTTP 返回状态 {}，未显示可能含敏感内容的响应体",
                    status.as_u16()
                )
            });
        }
        let session = response.headers().get("mcp-session-id").cloned();
        let mut bytes = vec![];
        let mut found = None;
        while let Some(chunk) = response.chunk().await.map_err(|_| "读取响应失败")? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > 1024 * 1024 {
                return Err("响应超过 1 MB，检查已停止".into());
            }
            if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
                found = Some(v);
                break;
            }
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    if let Ok(v) = serde_json::from_str::<Value>(data.trim()) {
                        if v.get("id") == Some(&json!(1)) {
                            found = Some(v);
                            break;
                        }
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        let value = found.ok_or("未返回可识别的 JSON-RPC / SSE 初始化结果")?;
        let message = validated(&value)?;
        let mut notification = client
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .header(
                "MCP-Protocol-Version",
                value["result"]["protocolVersion"]
                    .as_str()
                    .unwrap_or("2025-03-26"),
            )
            .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        for (k, v) in &headers {
            notification = notification.header(k, v);
        }
        if let Some(s) = &session {
            notification = notification.header("Mcp-Session-Id", s);
        }
        let _ = notification.send().await;
        if let Some(s) = session {
            let mut delete = client.delete(&url).header("Mcp-Session-Id", s);
            for (k, v) in &headers {
                delete = delete.header(k, v);
            }
            let _ = delete.send().await;
        }
        Ok(format!(
            "{message}。仅完成协议初始化；原生 Agent 的登录、配置加载和工具调用仍需分别确认。"
        ))
    }
}
pub(crate) fn limited_environment(cmd: &mut tokio::process::Command) {
    cmd.env_clear();
    for key in [
        "PATH",
        "HOME",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "USER",
        "LOGNAME",
        "SHELL",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "SYSTEMROOT",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "TEMP",
        "TMP",
        "COMSPEC",
        "PATHEXT",
        "WINDIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
}
fn validated(v: &Value) -> Result<String> {
    if v.get("error").is_some() {
        return Err("服务器返回 JSON-RPC 错误，错误内容已隐藏；请在原生 Agent 中排查".into());
    }
    if v.get("id") != Some(&json!(1))
        || v.get("result")
            .and_then(|r| r.get("protocolVersion"))
            .and_then(Value::as_str)
            .is_none()
    {
        return Err("不是有效 MCP 初始化响应".into());
    }
    Ok("已收到有效 MCP 初始化响应".into())
}
