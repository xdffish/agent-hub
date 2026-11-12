use crate::model::*;
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
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
pub fn valid_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 120
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        || name == "."
        || name == ".."
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
    fs::create_dir_all(path).map_err(|_| "无法创建应用数据目录".to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| "无法设置目录权限".to_string())
}
pub fn atomic_write(path: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    let parent = path.parent().ok_or("路径没有父目录")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建目标目录".to_string())?;
    let tmp = parent.join(format!(".agent-hub-{}", id()));
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(if executable { 0o700 } else { 0o600 })
            .open(&tmp)
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
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    ensure_no_symlink_ancestors(path)?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
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
        .map(|m| m.file_type().is_symlink())
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
            if item.file_type().is_symlink() {
                return Err("资源含符号链接；请导入只含普通文件的 Skill 目录".into());
            }
            if item.file_type().is_dir() {
                if item.path() != path {
                    directories.push(path_string(
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
                path: path_string(relative),
                bytes: fs::read(item.path()).map_err(|_| "读取文件失败")?,
                executable: m.permissions().mode() & 0o111 != 0,
            });
        }
    } else {
        if meta.len() > MAX_BYTES {
            return Err("文件超过 20 MB".into());
        }
        entries.push(Entry {
            path: String::new(),
            bytes: fs::read(path).map_err(|_| "读取文件失败")?,
            executable: meta.permissions().mode() & 0o111 != 0,
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
            && (e.path.is_empty()
                || p.is_absolute()
                || p.components().any(|c| !matches!(c, Component::Normal(_))))
        {
            return Err("资源含非法相对路径".into());
        }
        if !names.insert(&e.path) {
            return Err("资源存在重复路径".into());
        }
        size += e.bytes.len() as u64;
    }
    for d in &s.directories {
        let p = Path::new(d);
        if d.is_empty()
            || p.is_absolute()
            || p.components().any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("资源含非法目录路径".into());
        }
    }
    if s.directory {
        let files: std::collections::BTreeSet<_> =
            s.entries.iter().map(|e| Path::new(&e.path)).collect();
        for p in s
            .entries
            .iter()
            .map(|e| Path::new(&e.path))
            .chain(s.directories.iter().map(Path::new))
        {
            if p.ancestors().skip(1).any(|a| files.contains(a))
                || (s.directories.iter().any(|d| Path::new(d) == p) && files.contains(p))
            {
                return Err("文件与目录路径冲突".into());
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
        .map(|m| m.file_type().is_symlink())
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
            .map(|m| m.file_type().is_symlink())
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
