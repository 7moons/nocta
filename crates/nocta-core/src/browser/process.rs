//! Own only processes launched by Nocta. Never terminate by executable name.
use crate::{CoreError, CoreResult};
use std::path::Path;

mod platform {
    use super::*;
    use std::{
        mem::{size_of, zeroed},
        os::windows::{
            ffi::OsStrExt,
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
        },
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::*,
        System::{JobObjects::*, Threading::*},
        UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL},
    };

    pub struct OwnedProcess {
        job: OwnedHandle,
        process: OwnedHandle,
        pid: u32,
    }
    fn quote(argument: &str) -> String {
        let mut result = String::from("\"");
        let mut slashes = 0;
        for character in argument.chars() {
            if character == '\\' {
                slashes += 1;
                continue;
            }
            if character == '"' {
                result.push_str(&"\\".repeat(slashes * 2 + 1));
            } else {
                result.push_str(&"\\".repeat(slashes));
            }
            result.push(character);
            slashes = 0;
        }
        result.push_str(&"\\".repeat(slashes * 2));
        result.push('"');
        result
    }
    fn handle(value: HANDLE) -> CoreResult<OwnedHandle> {
        if value.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: a newly-created, unique Win32 handle is transferred into RAII ownership.
        Ok(unsafe { OwnedHandle::from_raw_handle(value) })
    }
    impl OwnedProcess {
        pub fn spawn(executable: &Path, args: &[String], visible: bool) -> CoreResult<Self> {
            let application: Vec<u16> = executable
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            let line = std::iter::once(quote(&executable.to_string_lossy()))
                .chain(args.iter().map(|argument| quote(argument)))
                .collect::<Vec<_>>()
                .join(" ");
            let mut line: Vec<u16> = line.encode_utf16().chain(Some(0)).collect();
            // SAFETY: all structures are initialized; pointers stay live until the synchronous APIs return.
            unsafe {
                let job = handle(CreateJobObjectW(null(), null()))?;
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                let mut startup: STARTUPINFOW = zeroed();
                startup.cb = size_of::<STARTUPINFOW>() as u32;
                startup.dwFlags = STARTF_USESHOWWINDOW;
                startup.wShowWindow = if visible {
                    SW_SHOWNORMAL as u16
                } else {
                    SW_HIDE as u16
                };
                let mut info: PROCESS_INFORMATION = zeroed();
                if CreateProcessW(
                    application.as_ptr(),
                    line.as_mut_ptr(),
                    null(),
                    null(),
                    0,
                    CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
                    null(),
                    null(),
                    &startup,
                    &mut info,
                ) == 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                let process = handle(info.hProcess)?;
                let thread = handle(info.hThread)?;
                if AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) == 0 {
                    let error = std::io::Error::last_os_error();
                    TerminateProcess(process.as_raw_handle(), 1);
                    return Err(error.into());
                }
                if ResumeThread(thread.as_raw_handle()) == u32::MAX {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(Self {
                    job,
                    process,
                    pid: info.dwProcessId,
                })
            }
        }
        pub fn id(&self) -> u32 {
            self.pid
        }
        pub fn running(&mut self) -> CoreResult<bool> {
            let mut code = 0;
            // SAFETY: this handle remains owned and valid throughout the query.
            unsafe {
                if GetExitCodeProcess(self.process.as_raw_handle(), &mut code) == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            Ok(code == STILL_ACTIVE as u32)
        }
        pub fn terminate(&mut self) -> CoreResult<()> {
            // SAFETY: this job contains only processes created suspended and assigned by this owner.
            unsafe {
                if TerminateJobObject(self.job.as_raw_handle(), 0) == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            Ok(())
        }
        pub fn tree_stopped(&mut self) -> CoreResult<bool> {
            // SAFETY: the initialized structure and its size match the selected information class.
            unsafe {
                let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = zeroed();
                if QueryInformationJobObject(
                    self.job.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    null_mut(),
                ) == 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(info.ActiveProcesses == 0)
            }
        }
    }
}

pub use platform::OwnedProcess;
impl OwnedProcess {
    pub async fn shutdown(&mut self) -> CoreResult<()> {
        self.terminate()?;
        for _ in 0..100 {
            if self.tree_stopped()? {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Err(CoreError::Browser(
            "The managed process tree did not stop in time.".into(),
        ))
    }
}
