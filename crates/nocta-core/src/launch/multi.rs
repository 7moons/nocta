//! Windows multi-instance helper: owns the Roblox singleton mutex on a dedicated thread.
//!
//! Mutex ownership belongs to the acquiring thread, so a lifetime thread creates, holds and releases it. A mutex
//! already owned by another process (for example a running Roblox client) is reported as busy and never forced.
use crate::{models::MultiInstanceState, CoreResult};

pub const MUTEX_NAME: &str = "ROBLOX_singletonEvent";

#[derive(Default)]
pub struct MultiInstance {
    state: MultiInstanceState,
    detail: Option<String>,
    holder: Option<windows::Holder>,
}

impl MultiInstance {
    pub fn state(&self) -> MultiInstanceState {
        if self.holder.as_ref().is_some_and(|holder| holder.finished()) {
            return MultiInstanceState::Failed;
        }
        self.state
    }
    pub fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    pub fn acquire(&mut self) -> CoreResult<MultiInstanceState> {
        if matches!(
            self.state(),
            MultiInstanceState::Acquired | MultiInstanceState::Abandoned
        ) {
            return Ok(self.state);
        }
        self.release();
        let (state, holder) = windows::Holder::start()?;
        self.state = state;
        self.holder = holder;
        self.detail = match state {
            MultiInstanceState::Acquired => None,
            MultiInstanceState::Abandoned => Some(
                "The singleton lock was abandoned by a previous owner and is now held by Nocta.".into(),
            ),
            MultiInstanceState::Busy => Some(
                "Another process holds the Roblox singleton lock. Close running Roblox clients and enable multi-instance again.".into(),
            ),
            _ => Some("The Roblox singleton lock could not be created.".into()),
        };
        Ok(state)
    }

    pub fn release(&mut self) {
        if let Some(holder) = self.holder.take() {
            holder.stop();
        }
        self.state = MultiInstanceState::Disabled;
        self.detail = None;
    }
}

impl Drop for MultiInstance {
    fn drop(&mut self) {
        self.release();
    }
}

mod windows {
    use super::MUTEX_NAME;
    use crate::{models::MultiInstanceState, CoreError, CoreResult};
    use std::{sync::mpsc, thread::JoinHandle, time::Duration};
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, WAIT_ABANDONED, WAIT_OBJECT_0,
        },
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    };

    pub struct Holder {
        stop: mpsc::Sender<()>,
        thread: JoinHandle<()>,
    }

    impl Holder {
        pub fn start() -> CoreResult<(MultiInstanceState, Option<Self>)> {
            Self::start_named(MUTEX_NAME)
        }

        fn start_named(name: &str) -> CoreResult<(MultiInstanceState, Option<Self>)> {
            let name = name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let (report, ready) = mpsc::channel();
            let (stop, stopped) = mpsc::channel::<()>();
            let thread = std::thread::Builder::new()
                .name("nocta-multi-instance".into())
                .spawn(move || hold(name, report, stopped))
                .map_err(CoreError::Io)?;
            let state = ready
                .recv_timeout(Duration::from_secs(3))
                .unwrap_or(MultiInstanceState::Failed);
            if matches!(
                state,
                MultiInstanceState::Acquired | MultiInstanceState::Abandoned
            ) {
                Ok((state, Some(Self { stop, thread })))
            } else {
                let _ = stop.send(());
                let _ = thread.join();
                Ok((state, None))
            }
        }
        pub fn finished(&self) -> bool {
            self.thread.is_finished()
        }
        pub fn stop(self) {
            let _ = self.stop.send(());
            let _ = self.thread.join();
        }
    }

    fn hold(name: Vec<u16>, report: mpsc::Sender<MultiInstanceState>, stopped: mpsc::Receiver<()>) {
        // SAFETY: valid NUL-terminated name, default security attributes, handle closed on this thread.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            let _ = report.send(MultiInstanceState::Failed);
            return;
        }
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let state = if !existed {
            MultiInstanceState::Acquired
        } else {
            // An existing mutex is not owned by us yet; only an immediate wait may take it.
            match unsafe { WaitForSingleObject(handle, 0) } {
                WAIT_OBJECT_0 => MultiInstanceState::Acquired,
                WAIT_ABANDONED => MultiInstanceState::Abandoned,
                _ => MultiInstanceState::Busy,
            }
        };
        let owned = matches!(
            state,
            MultiInstanceState::Acquired | MultiInstanceState::Abandoned
        );
        let _ = report.send(state);
        if owned {
            let _ = stopped.recv();
            unsafe { ReleaseMutex(handle) };
        }
        unsafe { CloseHandle(handle) };
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn second_holder_is_busy_and_release_frees_the_lock() {
            // Run against a private test name to avoid touching a real Roblox client lock.
            let name = format!("Local\\nocta-test-{}", uuid::Uuid::new_v4());
            let (state, first) = Holder::start_named(&name).unwrap();
            assert_eq!(state, MultiInstanceState::Acquired);
            let (state, second) = Holder::start_named(&name).unwrap();
            assert_eq!(state, MultiInstanceState::Busy);
            assert!(second.is_none());
            first.unwrap().stop();
            let (state, reacquired) = Holder::start_named(&name).unwrap();
            assert_eq!(state, MultiInstanceState::Acquired);
            reacquired.unwrap().stop();
        }
    }
}
