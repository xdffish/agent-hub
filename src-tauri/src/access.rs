use crate::{app::Hub, files::*, model::*};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PathPurpose {
    Project,
    Source,
    Import,
    Export,
}

#[derive(Clone)]
pub struct Grant {
    pub purpose: PathPurpose,
    pub path: PathBuf,
}

pub async fn confirm(app: &AppHandle, title: &str, message: String) -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancel)
        .show(move |yes| {
            let _ = tx.send(yes);
        });
    if rx.await.map_err(|_| "原生确认已关闭")? {
        Ok(())
    } else {
        Err("已取消；未执行操作".into())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn choose_path(
    app: AppHandle,
    hub: State<'_, Hub>,
    purpose: PathPurpose,
) -> Result<Option<String>> {
    let _operation = hub.operation();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let builder = app.dialog().file();
    match purpose {
        PathPurpose::Project => builder
            .set_title("选择要管理的项目目录")
            .pick_folder(move |p| {
                let _ = tx.send(p);
            }),
        PathPurpose::Source => builder
            .set_title("选择 Skill 来源目录")
            .pick_folder(move |p| {
                let _ = tx.send(p);
            }),
        PathPurpose::Import => builder
            .set_title("选择 Agent Hub 备份")
            .add_filter("Agent Hub 备份", &["json"])
            .pick_file(move |p| {
                let _ = tx.send(p);
            }),
        PathPurpose::Export => builder
            .set_title("导出 Agent Hub 备份")
            .set_file_name("agent-hub-backup.json")
            .add_filter("Agent Hub 备份", &["json"])
            .save_file(move |p| {
                let _ = tx.send(p);
            }),
    }
    let Some(selected) = rx.await.map_err(|_| "文件选择器已关闭")? else {
        return Ok(None);
    };
    let selected = selected.into_path().map_err(|_| "只支持本地文件路径")?;
    let path = normalize_choice(&selected, purpose)?;
    hub.grant(purpose, path.clone())?;
    Ok(Some(path_string(&path)))
}

pub fn normalize_choice(path: &Path, purpose: PathPurpose) -> Result<PathBuf> {
    checked_path(&path_string(path))?;
    let p = if purpose == PathPurpose::Export {
        if path.exists() {
            return Err("导出路径已存在，请选择一个新文件名".into());
        }
        let parent = path
            .parent()
            .ok_or("路径无父目录")?
            .canonicalize()
            .map_err(|_| "导出目录不存在")?;
        parent.join(path.file_name().ok_or("缺少文件名")?)
    } else {
        path.canonicalize().map_err(|_| "所选路径已不存在")?
    };
    match purpose {
        PathPurpose::Project | PathPurpose::Source => {
            if !p.is_dir() {
                return Err("需要选择目录".into());
            }
        }
        PathPurpose::Import => {
            if !p.is_file() {
                return Err("需要选择备份文件".into());
            }
        }
        PathPurpose::Export => {}
    }
    ensure_no_symlink_ancestors(&p)?;
    Ok(p)
}

impl Hub {
    pub fn grant(&self, purpose: PathPurpose, path: PathBuf) -> Result<()> {
        let mut grants = self.grants.lock().map_err(|_| "路径授权锁不可用")?;
        grants.retain(|g| g.purpose != purpose || g.path != path);
        // A bounded session list; persistent project access is represented separately.
        if grants.len() >= 100 {
            grants.remove(0);
        }
        grants.push(Grant { purpose, path });
        Ok(())
    }
    pub fn require_grant(&self, purpose: PathPurpose, path: &Path) -> Result<()> {
        checked_path(&path_string(path))?;
        ensure_no_symlink_ancestors(path)?;
        let allowed = self
            .grants
            .lock()
            .map_err(|_| "路径授权锁不可用")?
            .iter()
            .any(|g| {
                g.purpose == purpose
                    && if purpose == PathPurpose::Source {
                        path.starts_with(&g.path)
                    } else {
                        path == g.path
                    }
            });
        if allowed {
            Ok(())
        } else {
            Err("路径尚未通过原生选择器授权，或授权用途不匹配；请重新选择".into())
        }
    }
    pub fn library_path(&self, skill: &Skill) -> Result<PathBuf> {
        if skill.id.len() != 64 || !skill.id.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("管理版本 ID 无效".into());
        }
        let expected = self.root()?.join("library").join(&skill.id);
        if Path::new(&skill.folder) != expected {
            return Err("管理版本路径不属于本应用；请重新导入资源".into());
        }
        ensure_no_symlink_ancestors(&expected)?;
        Ok(expected)
    }
    pub fn authorize_change(&self, change: &Change) -> Result<()> {
        let path = checked_path(&change.path)?;
        ensure_no_symlink_ancestors(&path)?;
        let root = self.root()?;
        if path == root.join("settings.json") {
            return Ok(());
        }
        if path.parent() == Some(root.join("library").as_path()) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.len() == 64 && name.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Ok(());
            }
        }
        let settings = self.settings()?;
        let home = self.home()?;
        for project in
            std::iter::once(None).chain(settings.projects.iter().map(|p| Some(p.as_str())))
        {
            let base = project
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.clone());
            if path == base.join(".claude/settings.json")
                || (project.is_some() && path == base.join(".claude/settings.local.json"))
            {
                return Ok(());
            }
            for agent in crate::scan::AGENTS {
                if path
                    == self.target(
                        &Target {
                            agent: agent.into(),
                            project: project.map(str::to_string),
                        },
                        false,
                    )?
                {
                    return Ok(());
                }
            }
            for (base, _) in crate::scan::skill_roots(&home, project) {
                if let Ok(relative) = path.strip_prefix(base) {
                    if relative.components().count() == 0
                        || relative.components().count() > 4
                        || relative.components().any(|c| c.as_os_str() == ".system")
                    {
                        continue;
                    }
                    let skill_snapshot = if change.after.exists {
                        &change.after
                    } else {
                        &change.before
                    };
                    if skill_snapshot.directory
                        && skill_snapshot.entries.iter().any(|e| e.path == "SKILL.md")
                    {
                        return Ok(());
                    }
                }
            }
        }
        Err(format!(
            "写入目标超出本机 Agent 配置与 Agent Hub 管理范围：{}",
            change.path
        ))
    }
}

pub fn allow_remote_url(url: &reqwest::Url) -> Result<()> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URL 不能内嵌用户名或密码".into());
    }
    let local = url.host_str().is_some_and(|h| {
        h == "localhost"
            || h.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
            || h == "[::1]"
    });
    if url.scheme() == "https" || (url.scheme() == "http" && local) {
        Ok(())
    } else {
        Err("远程 MCP 需要 HTTPS；HTTP 仅允许 localhost 或回环地址".into())
    }
}
