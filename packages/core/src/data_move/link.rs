//! The link at the default data path that points at a moved data folder: a
//! directory junction on Windows (no privilege needed), a symlink elsewhere.

use std::io;
use std::path::{Path, PathBuf};

/// Where the link at `at` points, or `None` when `at` is not a link.
pub fn target(at: &Path) -> Option<PathBuf> {
    at.symlink_metadata()
        .ok()
        .filter(|m| m.file_type().is_symlink())?;
    let target = std::fs::read_link(at).ok()?;
    let target = match target.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => target,
    };
    if target.is_absolute() {
        Some(target)
    } else {
        Some(at.parent()?.join(target))
    }
}

/// The folder the data really lives in: where the link at `root` points, or
/// `root` itself when it is not a link.
pub fn real_folder(root: &Path) -> PathBuf {
    target(root).unwrap_or_else(|| root.to_path_buf())
}

#[cfg(not(windows))]
pub fn create(target: &Path, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(windows)]
pub fn create(target: &Path, at: &Path) -> io::Result<()> {
    windows::junction(target, at)
}

/// Removes the link at `at`, never what it points at.
pub fn remove(at: &Path) -> io::Result<()> {
    std::fs::remove_dir(at).or_else(|_| std::fs::remove_file(at))
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    const GENERIC_WRITE: u32 = 0x4000_0000;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00A4;
    const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            device: *mut c_void,
            code: u32,
            input: *const c_void,
            input_size: u32,
            output: *mut c_void,
            output_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    pub fn junction(target: &Path, at: &Path) -> io::Result<()> {
        let target = std::path::absolute(target)?;
        let plain = target.to_string_lossy();
        let plain = plain.strip_prefix(r"\\?\").unwrap_or(&plain);
        let print: Vec<u16> = plain.encode_utf16().collect();
        let substitute: Vec<u16> = r"\??\".encode_utf16().chain(print.iter().copied()).collect();
        let bytes =
            |s: &[u16]| u16::try_from(s.len() * 2).map_err(|_| io::Error::other("the path is too long"));
        let (sub, prn) = (bytes(&substitute)?, bytes(&print)?);
        let mut data: Vec<u8> = Vec::new();
        data.extend(IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
        data.extend((8 + sub + 2 + prn + 2).to_le_bytes());
        data.extend([0, 0, 0, 0]);
        data.extend(sub.to_le_bytes());
        data.extend((sub + 2).to_le_bytes());
        data.extend(prn.to_le_bytes());
        for unit in substitute.iter().chain(&[0]).chain(&print).chain(&[0]) {
            data.extend(unit.to_le_bytes());
        }
        std::fs::create_dir(at)?;
        let wide: Vec<u16> = at.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: `wide` is NUL-terminated, the handle is checked before use and
        // closed once, and DeviceIoControl reads `data.len()` bytes of `data`.
        let set = unsafe {
            let dir = CreateFileW(
                wide.as_ptr(),
                GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            );
            if dir == INVALID_HANDLE_VALUE {
                Err(io::Error::last_os_error())
            } else {
                let mut returned = 0;
                let ok = DeviceIoControl(
                    dir,
                    FSCTL_SET_REPARSE_POINT,
                    data.as_ptr().cast(),
                    data.len() as u32,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                );
                let set = if ok != 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                };
                CloseHandle(dir);
                set
            }
        };
        if set.is_err() {
            std::fs::remove_dir(at).ok();
        }
        set
    }
}
