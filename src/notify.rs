//! TCP notification server for instant wake on message arrival.
//!
//! Used by the delivery loop to block efficiently instead of busy-polling.
//! When a message is sent (`hcom send`), `notify_all_instances()` connects
//! briefly to each instance's notify port to wake its delivery thread.
//!
//! TCP chosen for clean poll/select integration across process boundaries.

use anyhow::{Context, Result};
use std::net::TcpListener;
use std::time::Duration;

/// TCP notification server for wake-ups
pub struct NotifyServer {
    listener: TcpListener,
    port: u16,
}

impl NotifyServer {
    /// Create a new notify server bound to localhost on auto-assigned port
    pub fn new() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind notify server")?;
        let port = listener.local_addr()?.port();

        // Set non-blocking so all platforms can use accept loops safely.
        listener.set_nonblocking(true)?;

        Ok(Self { listener, port })
    }

    /// Get the port the server is listening on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Wait for notification or timeout
    ///
    /// Returns true if notified (connection received), false on timeout
    pub fn wait(&self, timeout: Duration) -> bool {
        #[cfg(unix)]
        {
            use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
            use std::os::fd::{AsRawFd, BorrowedFd};

            let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
            let poll_timeout = PollTimeout::try_from(timeout_ms).unwrap_or(PollTimeout::MAX);

            let fd = unsafe { BorrowedFd::borrow_raw(self.listener.as_raw_fd()) };
            let mut poll_fds = [PollFd::new(fd, PollFlags::POLLIN)];

            match poll(&mut poll_fds, poll_timeout) {
                Ok(n) if n > 0 => {
                    self.drain();
                    return true;
                }
                _ => return false,
            }
        }
        #[cfg(not(unix))]
        {
            let deadline = std::time::Instant::now() + timeout;
            loop {
                match self.listener.accept() {
                    Ok((stream, _)) => {
                        drop(stream);
                        self.drain();
                        return true;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        let now = std::time::Instant::now();
                        if now >= deadline {
                            return false;
                        }
                        let remaining = deadline.saturating_duration_since(now);
                        std::thread::sleep(remaining.min(Duration::from_millis(25)));
                    }
                    Err(_) => return false,
                }
            }
        }
    }

    /// Drain all pending connections (accept and close)
    fn drain(&self) {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    // Just accepting wakes us up; close immediately
                    drop(stream);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
    }
}
