//! Bounded interrupt handling even when a pipe consumer stops reading.

use std::io;
use std::time::{Duration, Instant};

use crate::error::StopError;
use crate::runtime::Cancellation;

pub(super) enum Destination {
    Stdout,
    Stderr,
}

pub(super) fn write(
    bytes: &[u8],
    cancel: &Cancellation,
    destination: Destination,
) -> Result<(), StopError> {
    // Give an active reader a short chance to finish the current record, including
    // a final incomplete-run JSON document after Ctrl-C. A stalled reader cannot
    // hold cancellation hostage; bytes already written cannot be retracted.
    let mut deadline = None;
    let mut check_interrupt = || {
        if cancel.interrupted() {
            let deadline =
                deadline.get_or_insert_with(|| Instant::now() + Duration::from_millis(250));
            if Instant::now() >= *deadline {
                return Err(StopError::Interrupted);
            }
        }
        Ok(())
    };

    #[cfg(unix)]
    {
        let fd = match destination {
            Destination::Stdout => libc::STDOUT_FILENO,
            Destination::Stderr => libc::STDERR_FILENO,
        };
        let _mode = Nonblocking::new(fd)?;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            check_interrupt()?;
            // SAFETY: remaining is readable for the bounded count; fd is an
            // inherited standard stream. Nonblocking mode prevents pipe stalls.
            let written =
                unsafe { libc::write(fd, remaining.as_ptr().cast(), remaining.len().min(65536)) };
            if written > 0 {
                remaining = &remaining[written as usize..];
            } else if written == 0 {
                return Err(io::Error::from(io::ErrorKind::WriteZero).into());
            } else {
                let err = io::Error::last_os_error();
                match err.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => wait_writable(fd)?,
                    _ => return Err(err.into()),
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        use std::io::Write;
        check_interrupt()?;
        match destination {
            Destination::Stdout => io::stdout().lock().write_all(bytes)?,
            Destination::Stderr => io::stderr().lock().write_all(bytes)?,
        }
    }
    if cancel.interrupted() {
        Err(StopError::Interrupted)
    } else {
        Ok(())
    }
}

#[cfg(unix)]
struct Nonblocking {
    fd: libc::c_int,
    original: libc::c_int,
}

#[cfg(unix)]
impl Nonblocking {
    fn new(fd: libc::c_int) -> io::Result<Self> {
        // SAFETY: fcntl operates on an inherited descriptor and takes no pointer
        // arguments. Preserve flags, including preexisting O_NONBLOCK.
        let original = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if original < 0 {
            return Err(io::Error::last_os_error());
        }
        if original & libc::O_NONBLOCK == 0
            && unsafe { libc::fcntl(fd, libc::F_SETFL, original | libc::O_NONBLOCK) } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { fd, original })
    }
}

#[cfg(unix)]
impl Drop for Nonblocking {
    fn drop(&mut self) {
        if self.original & libc::O_NONBLOCK == 0 {
            // SAFETY: restore inherited open-file-description flags on handled
            // interruption, so a parent sharing this stream is not changed.
            unsafe { libc::fcntl(self.fd, libc::F_SETFL, self.original) };
        }
    }
}

#[cfg(unix)]
fn wait_writable(fd: libc::c_int) -> io::Result<()> {
    let mut fd = libc::pollfd {
        fd,
        events: libc::POLLOUT,
        revents: 0,
    };
    // SAFETY: one initialized pollfd; the bounded timeout permits cancellation.
    if unsafe { libc::poll(&mut fd, 1, 20) } < 0 {
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
    // Let write report closure or other errors without losing their IO kinds.
    Ok(())
}
