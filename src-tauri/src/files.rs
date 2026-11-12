use crate::model::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_BYTES: u64 = 20 * 1024 * 1024;
pub fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
pub fn id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn fingerprint(s: &Snapshot) -> String {
    hash(&serde_json::to_vec(s).unwrap_or_default())
}
pub fn path_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
fn portable_path(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Exchange paths use '/' on every OS. Reject Windows aliases and drive paths
/// even on Unix so a bundle cannot change meaning when moved to another host.
fn valid_relative(value: &str) -> bool {
    !value.is_empty()
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
                && !windows_reserved(part)
        })
}
fn windows_reserved(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}
pub fn native_snapshot(mut snapshot: Snapshot) -> Snapshot {
    if cfg!(windows) {
        for entry in &mut snapshot.entries {
            entry.executable = false;
        }
    }
    snapshot
}
pub fn valid_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 120
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || windows_reserved(name)
    {
        return Err("名称仅允许字母、数字、点、短横线和下划线（不超过 120 字符）".into());
    }
    Ok(())
}
pub fn checked_path(value: &str) -> Result<PathBuf> {
    let p = PathBuf::from(value);
    if !p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("请使用不含 .. 的绝对路径".into());
    }
    Ok(p)
}
pub fn private_dir(path: &Path) -> Result<()> {
    crate::platform::private_dir(path).map_err(|_| "无法设置目录权限".to_string())
}
pub fn atomic_write(path: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    let parent = path.parent().ok_or("路径没有父目录")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建目标目录".to_string())?;
    let tmp = parent.join(format!(".agent-hub-{}", id()));
    use std::io::Write;
    let result = (|| {
        let mut file = crate::platform::create_private_file(&tmp, executable)
            .map_err(|_| "无法写入临时文件")?;
        file.write_all(bytes).map_err(|_| "写入文件失败")?;
        file.sync_all().map_err(|_| "同步文件失败")?;
        fs::rename(&tmp, path).map_err(|_| "替换文件失败")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    ensure_no_symlink_ancestors(path)?;
    let mut file = crate::platform::create_private_file(path, false)
        .map_err(|_| "无法创建导出文件；目标可能已存在或没有写入权限")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "导出写入失败；所选位置可能有未完成文件，请检查后选择新文件名".into())
}
pub fn snapshot(path: &Path) -> Result<Snapshot> {
    snapshot_inner(path, false)
}
pub fn source_snapshot(path: &Path) -> Result<Snapshot> {
    snapshot_inner(path, true)
}
fn snapshot_inner(path: &Path, exclude_git: bool) -> Result<Snapshot> {
    if path
        .symlink_metadata()
        .map(|m| crate::platform::is_link(&m))
        .unwrap_or(false)
    {
        return Err("目标为符号链接：请通过原始位置管理，避免影响其他 Agent".into());
    }
    if !path.exists() {
        return Ok(Snapshot {
            exists: false,
            directory: false,
            entries: vec![],
            directories: vec![],
        });
    }
    let meta = fs::metadata(path).map_err(|_| "读取元信息失败")?;
    if !meta.is_dir() && !meta.is_file() {
        return Err("不支持此文件类型".into());
    }
    let mut entries = vec![];
    let mut directories = vec![];
    let mut total = 0;
    if meta.is_dir() {
        for item in walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !exclude_git || e.file_name() != ".git")
        {
            let item = item.map_err(|_| "读取目录失败")?;
            if crate::platform::is_link(
                &fs::symlink_metadata(item.path()).map_err(|_| "读取文件元信息失败")?,
            ) {
                return Err("资源含符号链接；请导入只含普通文件的 Skill 目录".into());
            }
            if item.file_type().is_dir() {
                if item.path() != path {
                    directories.push(portable_path(
                        item.path().strip_prefix(path).map_err(|_| "路径错误")?,
                    ));
                }
                if directories.len() > 2000 {
                    return Err("目录数量超过 2000".into());
                }
                continue;
            }
            if !item.file_type().is_file() {
                return Err("资源含特殊文件".into());
            }
            let relative = item.path().strip_prefix(path).map_err(|_| "路径错误")?;
            let m = item.metadata().map_err(|_| "读取文件元信息失败")?;
            total += m.len();
            if total > MAX_BYTES || entries.len() >= 2000 {
                return Err("资源超过限制：20 MB 或 2000 个文件".into());
            }
            entries.push(Entry {
                path: portable_path(relative),
                bytes: fs::read(item.path()).map_err(|_| "读取文件失败")?,
                executable: crate::platform::executable(&m),
            });
        }
    } else {
        if meta.len() > MAX_BYTES {
            return Err("文件超过 20 MB".into());
        }
        entries.push(Entry {
            path: String::new(),
            bytes: fs::read(path).map_err(|_| "读取文件失败")?,
            executable: crate::platform::executable(&meta),
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    directories.sort();
    Ok(Snapshot {
        exists: true,
        directory: meta.is_dir(),
        entries,
        directories,
    })
}
pub fn validate_snapshot(s: &Snapshot) -> Result<()> {
    if !s.exists && (s.directory || !s.entries.is_empty() || !s.directories.is_empty()) {
        return Err("不存在的快照不能带内容".into());
    }
    if s.exists
        && !s.directory
        && (s.entries.len() != 1 || !s.entries[0].path.is_empty() || !s.directories.is_empty())
    {
        return Err("文件快照结构无效".into());
    }
    let mut size = 0u64;
    let mut names = std::collections::BTreeSet::new();
    for e in &s.entries {
        let p = Path::new(&e.path);
        if s.directory
            && (!valid_relative(&e.path)
                || p.is_absolute()
                || p.components().any(|c| !matches!(c, Component::Normal(_))))
        {
            return Err("资源含非法相对路径".into());
        }
        let key = if cfg!(windows) {
            e.path.to_uppercase()
        } else {
            e.path.clone()
        };
        if !names.insert(key) {
            return Err("资源存在重复路径".into());
        }
        size += e.bytes.len() as u64;
    }
    for d in &s.directories {
        let p = Path::new(d);
        if !valid_relative(d)
            || p.is_absolute()
            || p.components().any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("资源含非法目录路径".into());
        }
        let key = if cfg!(windows) {
            d.to_uppercase()
        } else {
            d.clone()
        };
        if !names.insert(key) {
            return Err("资源存在重复或冲突路径".into());
        }
    }
    if s.directory {
        let key = |p: &str| {
            if cfg!(windows) {
                p.to_uppercase()
            } else {
                p.to_string()
            }
        };
        let files: std::collections::BTreeSet<_> = s.entries.iter().map(|e| key(&e.path)).collect();
        for p in &names {
            let mut ancestor = p.as_str();
            while let Some((parent, _)) = ancestor.rsplit_once('/') {
                if files.contains(parent) {
                    return Err("文件与目录路径冲突".into());
                }
                ancestor = parent;
            }
        }
    }
    if s.directories.len() > 2000 {
        return Err("目录数量超过限制".into());
    }
    if size > MAX_BYTES || s.entries.len() > 2000 {
        return Err("资源超过大小限制".into());
    }
    Ok(())
}
pub fn restore(path: &Path, s: &Snapshot) -> Result<()> {
    validate_snapshot(s)?;
    if path
        .symlink_metadata()
        .map(|m| crate::platform::is_link(&m))
        .unwrap_or(false)
    {
        return Err("目标变成符号链接，已停止".into());
    }
    if !s.exists {
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(|_| "移除目标目录失败")?;
        } else if path.exists() {
            fs::remove_file(path).map_err(|_| "移除目标文件失败")?;
        }
        return Ok(());
    }
    if s.directory {
        // The caller checks the complete preimage and keeps a private backup before this replacement.
        if path.exists() {
            fs::remove_dir_all(path).map_err(|_| "替换目录失败")?;
        }
        fs::create_dir_all(path).map_err(|_| "创建资源目录失败")?;
        for d in &s.directories {
            fs::create_dir_all(path.join(d)).map_err(|_| "创建子目录失败")?;
        }
        for e in &s.entries {
            atomic_write(&path.join(&e.path), &e.bytes, e.executable)?;
        }
    } else {
        let e = s.entries.first().ok_or("空文件快照")?;
        atomic_write(path, &e.bytes, e.executable)?;
    }
    Ok(())
}
pub fn file_snapshot(text: String) -> Snapshot {
    Snapshot {
        exists: true,
        directory: false,
        entries: vec![Entry {
            path: String::new(),
            bytes: text.into_bytes(),
            executable: false,
        }],
        directories: vec![],
    }
}
pub fn text_of(s: &Snapshot) -> Result<String> {
    if !s.exists {
        return Ok(String::new());
    }
    String::from_utf8(s.entries.first().ok_or("文件为空")?.bytes.clone())
        .map_err(|_| "文件不是 UTF-8 文本".into())
}
pub fn ensure_no_symlink_ancestors(p: &Path) -> Result<()> {
    for ancestor in p.ancestors() {
        if ancestor
            .symlink_metadata()
            .map(|m| crate::platform::is_link(&m))
            .unwrap_or(false)
        {
            return Err(format!(
                "路径包含符号链接，写入已停止：{}",
                path_string(ancestor)
            ));
        }
    }
    Ok(())
}
