//! A process's end on macOS: a kqueue with its `NOTE_EXIT` registered, waited on until it
//! fires.

use std::io;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};

/// A kqueue that holds one event.
struct Queue(OwnedFd);

impl Queue {
    /// A kqueue told of `filter` on `ident` with `fflags`.
    fn watching(ident: libc::uintptr_t, filter: i16, fflags: u32) -> io::Result<Self> {
        // SAFETY: kqueue takes nothing; a negative answer is an error.
        let made = unsafe { libc::kqueue() };
        if made < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `made` is a descriptor of this process's, owned from here on.
        let queue = Self(unsafe { OwnedFd::from_raw_fd(made) });
        let change = libc::kevent {
            ident,
            filter,
            flags: libc::EV_ADD | libc::EV_CLEAR,
            fflags,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: one change read from `change`; no events asked back, so no buffer is written.
        let added = unsafe {
            libc::kevent(
                queue.0.as_raw_fd(),
                &raw const change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if added < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(queue)
    }

    /// Until the event fires.
    fn wait(&self) -> io::Result<()> {
        // SAFETY: a kevent is plain data, all zeros a valid value.
        let mut fired: libc::kevent = unsafe { std::mem::zeroed() };
        loop {
            // SAFETY: no changes given; at most one event written into `fired`, no deadline.
            let got = unsafe {
                libc::kevent(
                    self.0.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    &raw mut fired,
                    1,
                    std::ptr::null(),
                )
            };
            match got {
                1.. => return Ok(()),
                0 => {}
                _ => {
                    let why = io::Error::last_os_error();
                    if why.kind() != io::ErrorKind::Interrupted {
                        return Err(why);
                    }
                }
            }
        }
    }
}

/// A process's end.
pub(super) struct Ending(Queue);

impl Ending {
    /// Process `pid`'s end, to wait on: an error when it is gone already.
    pub(super) fn of(pid: libc::pid_t) -> io::Result<Self> {
        let ident = libc::uintptr_t::try_from(pid).map_err(io::Error::other)?;
        Queue::watching(ident, libc::EVFILT_PROC, libc::NOTE_EXIT).map(Self)
    }

    pub(super) fn wait(&self) -> io::Result<()> {
        self.0.wait()
    }
}
