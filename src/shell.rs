//! Opening files, folders and web links with the system.
//!
//! - Open path: a folder opens with verb "explore" and class "folder"; anything else with its default
//!   action. Failure answers "Failed to open path".
//! - Shell execute: SHELLEXECUTEINFOW cbSize 0x70, fMask SEE_MASK_NOASYNC 0x100 (| 0x1 SEE_MASK_CLASSNAME with a
//!   class), nShow SW_SHOWNORMAL 1, no verb = the default action; success = ShellExecuteExW.
//! - Show item in folder: on a COM worker thread, parse the folder and the file to item lists and call
//!   SHOpenFolderAndSelectItems(folder, 1, [file], 0).
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
        let ok = if p.is_dir() { win::shell_execute(p.as_os_str(), Some("explore"), Some("folder"), SEE_MASK_NOASYNC) } else { win::shell_execute(p.as_os_str(), None, None, SEE_MASK_NOASYNC) };
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
        let folder = p.parent().ok_or_else(|| OPEN_PATH_FAILED.to_string())?.to_path_buf();
        let file = p.to_path_buf();
        // On its own thread with COM set up for it.
        std::thread::spawn(move || win::open_folder_and_select(&folder, &file)).join().map_err(|_| OPEN_PATH_FAILED.to_string())?
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg("-R").arg(p).spawn().map(|_| ()).map_err(|_| OPEN_PATH_FAILED.into())
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(p.parent().unwrap_or(p)).spawn().map(|_| ()).map_err(|_| OPEN_PATH_FAILED.into())
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
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows_sys::Win32::UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName, ShellExecuteExW, SHELLEXECUTEINFOW};

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
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
        // SAFETY: info is fully initialised above.
        if unsafe { ShellExecuteExW(&mut info) } != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32)
        }
    }

    /// Show an item in its folder (runs on a COM worker thread).
    pub fn open_folder_and_select(folder: &Path, file: &Path) -> Result<(), String> {
        // SAFETY: plain COM calls on this thread; every item list is freed and COM is closed if it was opened here.
        unsafe {
            let com = CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as _);
            let parse = |p: &Path| {
                let w = wide(p.as_os_str());
                let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
                let hr = SHParseDisplayName(w.as_ptr(), std::ptr::null_mut(), &mut pidl, 0, std::ptr::null_mut());
                if hr >= 0 { Some(pidl) } else { None }
            };
            let dir = parse(folder);
            let item = parse(file);
            let result = match (dir, item) {
                (Some(d), Some(i)) => {
                    let items = [i as *const ITEMIDLIST];
                    if SHOpenFolderAndSelectItems(d, 1, items.as_ptr(), 0) >= 0 { Ok(()) } else { Err(OPEN_PATH_FAILED.to_string()) }
                }
                _ => Err(OPEN_PATH_FAILED.to_string()),
            };
            for p in [dir, item].into_iter().flatten() {
                CoTaskMemFree(p as *const _);
            }
            if com >= 0 {
                CoUninitialize();
            }
            result
        }
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
