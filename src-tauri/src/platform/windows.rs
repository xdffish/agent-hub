use std::{
    ffi::c_void,
    fs, io, mem,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{LocalFree, GENERIC_WRITE, INVALID_HANDLE_VALUE},
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{Diagnostics::ToolHelp::*, JobObjects::*, Threading::*},
};

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let value: Vec<_> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"));
    }
    let absolute = std::path::absolute(path)?;
    let value: Vec<_> = absolute.as_os_str().encode_wide().collect();
    let prefix: Vec<_> = "\\\\?\\".encode_utf16().collect();
    let extended = if value.starts_with(&prefix) {
        value
    } else if value.starts_with(&[92, 92]) {
        "\\\\?\\UNC\\"
            .encode_utf16()
            .chain(value.into_iter().skip(2))
            .collect()
    } else {
        prefix.into_iter().chain(value).collect()
    };
    Ok(extended.into_iter().chain(Some(0)).collect())
}

struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

/// A protected DACL grants access to the current user and SYSTEM only.
/// Files receive it at creation, including when exported outside the private data directory.
fn descriptor(directory: bool) -> io::Result<Local> {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle::from_raw_handle(token);
        let mut length = 0;
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &mut length,
        );
        // usize storage preserves TOKEN_USER's alignment.
        let mut data = vec![0usize; (length as usize).div_ceil(mem::size_of::<usize>())];
        if GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            data.as_mut_ptr().cast(),
            length,
            &mut length,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*(data.as_ptr().cast::<TOKEN_USER>());
        let mut sid = ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid_memory = Local(sid.cast());
        let mut len = 0;
        while *sid.add(len) != 0 {
            len += 1;
        }
        let sid_string = String::from_utf16_lossy(std::slice::from_raw_parts(sid, len));
        drop(sid_memory);
        let inherit = if directory { "OICI" } else { "" };
        let sddl: Vec<u16> = format!("D:P(A;{inherit};FA;;;SY)(A;{inherit};FA;;;{sid_string})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut sd = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut sd,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Local(sd))
    }
}

pub fn private_dir(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing parent"))?;
    fs::create_dir_all(parent)?;
    let sd = descriptor(true)?;
    let attrs = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: 0,
    };
    let path = wide(path)?;
    unsafe {
        if CreateDirectoryW(path.as_ptr(), &attrs) == 0
            && io::Error::last_os_error().raw_os_error() != Some(183)
        {
            return Err(io::Error::last_os_error());
        }
        let attributes = GetFileAttributesW(path.as_ptr());
        if attributes == INVALID_FILE_ATTRIBUTES {
            return Err(io::Error::last_os_error());
        }
        if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Expected private directory",
            ));
        }
        let mut dacl = ptr::null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        if GetSecurityDescriptorDacl(sd.0, &mut present, &mut dacl, &mut defaulted) == 0
            || present == 0
            || dacl.is_null()
        {
            return Err(io::Error::last_os_error());
        }
        let status = SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            dacl,
            ptr::null(),
        );
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
    }
    Ok(())
}

pub fn create_private_file(path: &Path, _executable: bool) -> io::Result<fs::File> {
    let sd = descriptor(false)?;
    let attrs = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: 0,
    };
    let path = wide(path)?;
    unsafe {
        let handle = CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            &attrs,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(fs::File::from_raw_handle(handle))
    }
}

// Closing the last job handle terminates every process in the temporary tree.
pub struct ProcessTree(OwnedHandle);

pub fn spawn(
    cmd: &mut tokio::process::Command,
) -> io::Result<(tokio::process::Child, ProcessTree)> {
    unsafe {
        let handle = CreateJobObjectW(ptr::null(), ptr::null());
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = ProcessTree(OwnedHandle::from_raw_handle(handle));
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            mem::size_of_val(&limits) as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        // Suspend before assignment so the process cannot create an untracked descendant.
        cmd.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW)
            .kill_on_drop(true);
        let child = cmd.spawn()?;
        let process = child
            .raw_handle()
            .ok_or_else(|| io::Error::other("Missing child process handle"))?;
        if AssignProcessToJobObject(job.0.as_raw_handle(), process) == 0 {
            return Err(io::Error::last_os_error());
        }
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = OwnedHandle::from_raw_handle(snapshot);
        let mut entry: THREADENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of_val(&entry) as u32;
        let mut found = Thread32First(snapshot.as_raw_handle(), &mut entry);
        while found != 0 {
            if Some(entry.th32OwnerProcessID) == child.id() {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let thread = OwnedHandle::from_raw_handle(thread);
                if ResumeThread(thread.as_raw_handle()) == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                return Ok((child, job));
            }
            found = Thread32Next(snapshot.as_raw_handle(), &mut entry);
        }
        Err(io::Error::other("Missing suspended process thread"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn dacl_string(sd: PSECURITY_DESCRIPTOR) -> String {
        let mut text = ptr::null_mut();
        assert_ne!(
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                sd,
                1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                ptr::null_mut()
            ),
            0
        );
        let _allocation = Local(text.cast());
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
    }

    #[test]
    fn private_paths_have_protected_user_and_system_dacls() {
        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("private");
        private_dir(&private).unwrap();
        // Also repair permissions on an existing private directory.
        private_dir(&private).unwrap();
        let file = private.join("backup.json");
        drop(create_private_file(&file, false).unwrap());
        assert!(create_private_file(&file, false).is_err());
        for (path, directory) in [(&private, true), (&file, false)] {
            unsafe {
                let path = wide(path).unwrap();
                let mut sd = ptr::null_mut();
                let status = GetNamedSecurityInfoW(
                    path.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut sd,
                );
                assert_eq!(status, 0);
                let actual = Local(sd);
                let expected = descriptor(directory).unwrap();
                let actual = dacl_string(actual.0);
                let expected = dacl_string(expected.0);
                assert!(actual.starts_with("D:P"));
                assert_eq!(
                    actual.split('(').skip(1).collect::<Vec<_>>(),
                    expected.split('(').skip(1).collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn long_paths_and_cmd_shims_are_supported() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir
            .path()
            .join("long-directory-name".repeat(8))
            .join("nested-directory".repeat(8));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("backup.json");
        drop(create_private_file(&path, false).unwrap());
        assert!(path.is_file());
        let shim = dir.path().join("npx.cmd");
        fs::write(&shim, "@echo off\r\n").unwrap();
        let search = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(
            crate::platform::command_path("npx", None, Some(&search)),
            shim
        );
    }

    #[test]
    fn junctions_cannot_bypass_path_authorization() {
        use std::os::windows::process::CommandExt;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("junction");
        fs::create_dir(&target).unwrap();
        let status = std::process::Command::new("cmd.exe")
            .args(["/D", "/V:OFF", "/C"])
            .raw_arg(r#"mklink /J "%AGENT_HUB_LINK%" "%AGENT_HUB_TARGET%""#)
            .env("AGENT_HUB_LINK", &link)
            .env("AGENT_HUB_TARGET", &target)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(crate::platform::is_link(
            &fs::symlink_metadata(&link).unwrap()
        ));
        assert!(crate::files::snapshot(&link).is_err());
        assert!(crate::files::ensure_no_symlink_ancestors(&link.join("child")).is_err());
        fs::remove_dir(&link).unwrap();
        assert!(target.exists());
    }
}
