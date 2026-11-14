//! A cancellable process group on Unix and a kill-on-close job on Windows.
use std::process::{Child, Command};

pub fn prepare(command: &mut Command) {
    #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
    #[cfg(not(unix))] { let _ = command; }
}

pub struct ProcessTree { id: usize }
impl ProcessTree {
    pub fn attach(child: &Child) -> Result<Self, String> {
        #[cfg(windows)] unsafe {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() { return Err(std::io::Error::last_os_error().to_string()); }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as _, std::mem::size_of_val(&info) as u32) == 0 || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0 {
                let error = std::io::Error::last_os_error().to_string();
                CloseHandle(job);
                return Err(error);
            }
            Ok(Self { id: job as usize })
        }
        #[cfg(not(windows))] { Ok(Self { id: child.id() as usize }) }
    }
    pub fn kill(&self) -> bool {
        #[cfg(windows)] unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.id as _, 1) != 0 }
        #[cfg(unix)] unsafe {
            extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
            kill(-(self.id as i32), 9) == 0
        }
        #[cfg(not(any(windows, unix)))] { false }
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        #[cfg(windows)] unsafe { windows_sys::Win32::Foundation::CloseHandle(self.id as _); }
        #[cfg(unix)] { self.kill(); }
    }
}
