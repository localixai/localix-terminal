//! A process's end on Linux: its pidfd, which becomes readable as the process ends.

use std::io;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};

/// A process's end: its pidfd.
pub(super) struct Ending(OwnedFd);

impl Ending {
    /// Process `pid`'s end, to wait on: an error when it is gone already, or the kernel has no
    /// pidfd.
    pub(super) fn of(pid: libc::pid_t) -> io::Result<Self> {
        // SAFETY: pidfd_open takes a pid and flags; a negative answer is an error.
        let opened = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        let fd = libc::c_int::try_from(opened).map_err(io::Error::other)?;
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` is a descriptor of this process's, owned from here on.
        Ok(Self(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    pub(super) fn wait(&self) -> io::Result<()> {
        let mut asked = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            // SAFETY: one pollfd, no deadline.
            let got = unsafe { libc::poll(&raw mut asked, 1, -1) };
            if got > 0 {
                return Ok(());
            }
            let why = io::Error::last_os_error();
            if got < 0 && why.kind() != io::ErrorKind::Interrupted {
                return Err(why);
            }
        }
    }
}
