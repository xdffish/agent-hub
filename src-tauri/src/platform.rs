//! OS-specific filesystem and process primitives. Callers retain preview and backup checks.
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(windows)]
#[path = "platform/windows.rs"]
mod windows;
#[cfg(unix)]
use std::io;
#[cfg(windows)]
pub use windows::{create_private_file, private_dir, spawn};

#[cfg(unix)]
pub fn private_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(unix)]
pub fn create_private_file(path: &Path, executable: bool) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(if executable { 0o700 } else { 0o600 })
        .open(path)
}

pub fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and other reparse points must not bypass path authorization.
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(unix)]
    {
        meta.file_type().is_symlink()
    }
}

pub fn executable(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        let _ = meta;
        false
    }
}

#[cfg(unix)]
pub struct ProcessTree(i32);
#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        if self.0 > 0 {
            // Only the process group created for this operation is terminated.
            unsafe {
                libc::kill(-self.0, libc::SIGKILL);
            }
        }
    }
}

#[cfg(unix)]
pub fn spawn(
    cmd: &mut tokio::process::Command,
) -> io::Result<(tokio::process::Child, ProcessTree)> {
    use std::os::unix::process::CommandExt;
    cmd.as_std_mut().process_group(0);
    let child = cmd.kill_on_drop(true).spawn()?;
    let group = ProcessTree(child.id().unwrap_or(0) as i32);
    Ok((child, group))
}

/// Application paths are hints only; discovery never launches an executable.
pub fn app_candidates(agent: &str, home: &Path) -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let name = match agent {
            "claude" => "Claude Code.app",
            "codex" => "Codex.app",
            "cursor" => "Cursor.app",
            _ => "Antigravity.app",
        };
        vec![
            PathBuf::from("/Applications").join(name),
            home.join("Applications").join(name),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        let names: &[&str] = match agent {
            "claude" => &["Claude/Claude.exe", "Claude Code/Claude Code.exe"],
            "codex" => &["Codex/Codex.exe"],
            "cursor" => &["cursor/Cursor.exe", "Cursor/Cursor.exe"],
            _ => &["Antigravity/Antigravity.exe"],
        };
        let mut roots = vec![home.join("AppData/Local/Programs")];
        for key in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(key) {
                let root = PathBuf::from(root);
                if key == "LOCALAPPDATA" {
                    roots.push(root.join("Programs"));
                }
                roots.push(root);
            }
        }
        let mut candidates: Vec<_> = roots
            .into_iter()
            .flat_map(|p| names.iter().map(move |n| p.join(n)))
            .collect();
        if agent == "claude" {
            candidates.push(home.join(".local/bin/claude.exe"));
        }
        candidates
    }
    #[cfg(target_os = "linux")]
    {
        let name = match agent {
            "antigravity" => "antigravity",
            other => other,
        };
        vec![
            PathBuf::from("/usr/bin").join(name),
            PathBuf::from("/usr/local/bin").join(name),
            PathBuf::from("/opt").join(name).join(name),
            PathBuf::from("/usr/share").join(name).join(name),
            home.join(".local/bin").join(name),
        ]
    }
}

pub fn executable_names(binary: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            format!("{binary}.exe"),
            format!("{binary}.cmd"),
            format!("{binary}.bat"),
            binary.into(),
        ]
    }
    #[cfg(unix)]
    {
        vec![binary.into()]
    }
}

pub fn command_path(
    command: &str,
    cwd: Option<&Path>,
    search_path: Option<&std::ffi::OsStr>,
) -> PathBuf {
    #[cfg(windows)]
    {
        let path = Path::new(command);
        if path.extension().is_none() {
            let names = executable_names(command);
            if path.components().count() > 1 {
                if let Some(found) = names
                    .iter()
                    .map(|n| cwd.map_or_else(|| PathBuf::from(n), |base| base.join(n)))
                    .find(|p| p.is_file())
                {
                    return found;
                }
            } else if let Some(paths) = search_path {
                for directory in std::env::split_paths(paths) {
                    if let Some(found) = names
                        .iter()
                        .map(|n| directory.join(n))
                        .find(|p| p.is_file())
                    {
                        return found;
                    }
                }
            }
        }
    }
    #[cfg(unix)]
    let _ = (cwd, search_path);
    PathBuf::from(command)
}
