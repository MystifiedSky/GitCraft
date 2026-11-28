//! Opening files, folders and web links with the system.
//!
//! - Open path: a folder opens with verb "explore" and class "folder"; anything else with its default
//!   action. Failure answers "Failed to open path".
//! - Shell execute: SHELLEXECUTEINFOW cbSize 0x70, fMask SEE_MASK_NOASYNC 0x100 (| 0x1 SEE_MASK_CLASSNAME with a
//!   class), nShow SW_SHOWNORMAL 1, no verb = the default action; success = ShellExecuteExW.
//! - Show item in folder: on a COM worker thread, parse the file into an absolute item list and call
//!   SHOpenFolderAndSelectItems(file, 0, null, 0).
//! - Open external: verb "open", fMask 0x500 (SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI), nShow 1; failure
//!   answers "Failed to open: " and the error code.

use std::path::Path;

/// The answer when a path can't be opened.
pub const OPEN_PATH_FAILED: &str = "Failed to open path";
/// The prefix when a web link can't be opened.
pub const OPEN_EXTERNAL_FAILED: &str = "Failed to open: ";

/// ShellExecuteEx flags and window state.
pub const SEE_MASK_CLASSNAME: u32 = 0x1;
pub const SEE_MASK_NOASYNC: u32 = 0x100;
pub const SEE_MASK_FLAG_NO_UI: u32 = 0x400;
pub const SW_SHOWNORMAL: i32 = 1;
/// SHELLEXECUTEINFOW size on 64-bit Windows (cbSize = 0x70).
pub const SHELLEXECUTEINFO_SIZE: u32 = 0x70;

/// Open a file with its usual app, or a folder in the file manager.
pub fn open_path(p: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let path = win::shell_path(p).map_err(|_| OPEN_PATH_FAILED.to_string())?;
        let ok = if p.is_dir() { win::shell_execute(&path, Some("explore"), Some("folder"), SEE_MASK_NOASYNC) } else { win::shell_execute(&path, None, None, SEE_MASK_NOASYNC) };
        if ok.is_ok() { Ok(()) } else { Err(OPEN_PATH_FAILED.into()) }
    }
    #[cfg(not(windows))]
    {
        let prog = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        std::process::Command::new(prog).arg(p).spawn().map(|_| ()).map_err(|_| OPEN_PATH_FAILED.into())
    }
}

/// Open the file's folder with the file selected.
pub fn show_item_in_folder(p: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let file = p.to_path_buf();
        // On its own thread with COM set up for it.
        std::thread::spawn(move || win::open_folder_and_select(&file)).join().map_err(|_| OPEN_PATH_FAILED.to_string())?
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg("-R").arg(p).spawn().map(|_| ()).map_err(|_| OPEN_PATH_FAILED.into())
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if let Ok(path) = std::path::absolute(p) {
            if let Ok(uri) = url::Url::from_file_path(path) {
                // GVariant string escaping is handled by JSON quoting for a percent-encoded file URI.
                let list = format!("[{}]", serde_json::Value::String(uri.to_string()));
                let status = std::process::Command::new("gdbus").args([
                    "call", "--session", "--timeout", "3", "--dest", "org.freedesktop.FileManager1",
                    "--object-path", "/org/freedesktop/FileManager1", "--method", "org.freedesktop.FileManager1.ShowItems", &list, "''",
                ]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
                if status.is_ok_and(|status| status.success()) { return Ok(()); }
            }
        }
        open_path(p.parent().unwrap_or(p))
    }
}

/// Open a web link in the default browser.
pub fn open_external(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::shell_execute(std::ffi::OsStr::new(url), Some("open"), None, SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI).map_err(|code| format!("{OPEN_EXTERNAL_FAILED}{code}"))
    }
    #[cfg(not(windows))]
    {
        let prog = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        std::process::Command::new(prog).arg(url).spawn().map(|_| ()).map_err(|e| format!("{OPEN_EXTERNAL_FAILED}{e}"))
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Component, Prefix};
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows_sys::Win32::UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName, ShellExecuteExW, SHELLEXECUTEINFOW};

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Give Explorer an absolute DOS or UNC path. Git emits forward slashes, and
    /// Windows filesystem paths can contain a verbatim prefix the shell rejects.
    pub(super) fn shell_path(path: &Path) -> std::io::Result<OsString> {
        let absolute = std::path::absolute(path)?;
        let mut units: Vec<u16> = absolute.as_os_str().encode_wide().collect();
        if let Some(Component::Prefix(prefix)) = absolute.components().next() {
            match prefix.kind() {
                Prefix::VerbatimDisk(_) => { units.drain(..4); }
                Prefix::VerbatimUNC(..) => { units.drain(2..8); }
                _ => {}
            }
        }
        for unit in &mut units {
            if *unit == u16::from(b'/') {
                *unit = u16::from(b'\\');
            }
        }
        Ok(OsString::from_wide(&units))
    }

    /// Run ShellExecuteEx. Err holds GetLastError's code.
    pub fn shell_execute(file: &OsStr, verb: Option<&str>, class: Option<&str>, mask: u32) -> Result<(), u32> {
        let file = wide(file);
        let verb = verb.map(|v| wide(OsStr::new(v)));
        let class = class.map(|c| wide(OsStr::new(c)));
        // SAFETY: a zeroed SHELLEXECUTEINFOW is valid (null handles and strings); the strings outlive the call.
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        debug_assert_eq!(info.cbSize, SHELLEXECUTEINFO_SIZE);
        info.fMask = mask | if class.is_some() { SEE_MASK_CLASSNAME } else { 0 };
        info.lpVerb = verb.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());
        info.lpFile = file.as_ptr();
        info.lpClass = class.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        info.nShow = SW_SHOWNORMAL;
        // Shell verbs can be implemented by COM extensions. An existing COM apartment
        // can be used, but only balance initialization when this call succeeded.
        let com = unsafe { CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as _) };
        // SAFETY: info is fully initialised above. Capture the error before closing COM.
        let result = if unsafe { ShellExecuteExW(&mut info) } != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32)
        };
        if com >= 0 {
            unsafe { CoUninitialize() };
        }
        result
    }

    /// Show an item in its folder (runs on a COM worker thread).
    pub fn open_folder_and_select(file: &Path) -> Result<(), String> {
        let path = shell_path(file).map_err(|_| OPEN_PATH_FAILED.to_string())?;
        // SAFETY: plain COM calls on this thread; every item list is freed and COM is closed if it was opened here.
        unsafe {
            let com = CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as _);
            if com < 0 {
                return Err(OPEN_PATH_FAILED.into());
            }
            let item = parse_item(&path);
            let result = item.and_then(|item| {
                // The single-item form accepts an absolute PIDL and selects it in its parent.
                let hr = SHOpenFolderAndSelectItems(item, 0, std::ptr::null(), 0);
                CoTaskMemFree(item.cast());
                if hr >= 0 { Ok(()) } else { Err(OPEN_PATH_FAILED.to_string()) }
            });
            CoUninitialize();
            result
        }
    }

    // The caller initializes COM and frees the returned item with CoTaskMemFree.
    unsafe fn parse_item(path: &OsStr) -> Result<*mut ITEMIDLIST, String> {
        let path = wide(path);
        let mut item = std::ptr::null_mut();
        let hr = unsafe {
            SHParseDisplayName(path.as_ptr(), std::ptr::null_mut(), &mut item, 0, std::ptr::null_mut())
        };
        if hr >= 0 { Ok(item) } else { Err(OPEN_PATH_FAILED.to_string()) }
    }

    #[cfg(test)]
    #[test]
    fn windows_shell_resolves_git_paths_and_verbatim_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert!(crate::git::run(dir.path(), &["init", "-q"]).ok);
        let root = crate::git::repo_root(dir.path()).unwrap();
        let file = crate::files::repo_file(&root, "nested dir/café.txt").unwrap();
        std::fs::create_dir(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "").unwrap();
        let absolute = file.canonicalize().unwrap();

        // Exercise the shell's own parser without opening Explorer in CI.
        let paths = [&root, &file, &absolute];
        let com = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as _) };
        assert!(com >= 0);
        for path in paths {
            let normalized = shell_path(path).unwrap();
            let item = unsafe { parse_item(&normalized) }.unwrap_or_else(|_| panic!("Explorer rejected {normalized:?}"));
            unsafe { CoTaskMemFree(item.cast()) };
        }
        unsafe { CoUninitialize() };
        assert_eq!(shell_path(Path::new(r"\\?\UNC\server\share\repo")).unwrap(), r"\\server\share\repo");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_values_are_as_expected() {
        #[cfg(windows)]
        assert_eq!(std::mem::size_of::<windows_sys::Win32::UI::Shell::SHELLEXECUTEINFOW>() as u32, SHELLEXECUTEINFO_SIZE);
        assert_eq!(SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI, 0x500);
        assert_eq!(SEE_MASK_NOASYNC | SEE_MASK_CLASSNAME, 0x101);
        assert_eq!(OPEN_PATH_FAILED, "Failed to open path");
    }
}
