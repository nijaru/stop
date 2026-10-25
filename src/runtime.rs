//! Cooperative cancellation for finite commands and prompt pipe termination.

use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn install() -> Result<Self, ctrlc::Error> {
        let cancel = Self::default();
        let flag = cancel.0.clone();
        ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed))?;
        Ok(cancel)
    }
    pub fn interrupted(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    /// Returns false on interruption; propagates early stdout closure.
    pub fn wait_until(&self, deadline: Instant) -> io::Result<bool> {
        loop {
            if stdout_closed() {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            if self.interrupted() {
                return Ok(false);
            }
            let Some(delay) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(true);
            };
            std::thread::sleep(delay.min(Duration::from_millis(20)));
        }
    }
}

#[cfg(unix)]
fn stdout_closed() -> bool {
    let mut fd = libc::pollfd {
        fd: libc::STDOUT_FILENO,
        events: libc::POLLOUT,
        revents: 0,
    };
    // SAFETY: fd points to one initialized pollfd; timeout zero never blocks.
    unsafe { libc::poll(&mut fd, 1, 0) > 0 && fd.revents & (libc::POLLERR | libc::POLLHUP) != 0 }
}
#[cfg(not(unix))]
fn stdout_closed() -> bool {
    false
}
