//! Who is at the other end of a unix socket, as the system says: a server serves only its own
//! user's processes, and a client talks only to its own user's server.

use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

/// The user id of the process at the other end of `stream`, when the system says.
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub(crate) fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: getpeereid writes the peer's ids into the two variables, which outlive the call.
    let got = unsafe { libc::getpeereid(stream.as_raw_fd(), &raw mut uid, &raw mut gid) };
    (got == 0).then_some(uid)
}

/// The user id of the process at the other end of `stream`, when the system says.
#[cfg(target_os = "linux")]
pub(crate) fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut size = libc::socklen_t::try_from(size_of::<libc::ucred>()).ok()?;
    // SAFETY: getsockopt writes at most `size` bytes of the peer's credentials into `cred`.
    let got = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &raw mut size,
        )
    };
    (got == 0).then_some(cred.uid)
}

/// The process at the other end of `stream`, when the system says.
#[cfg(target_os = "macos")]
pub(crate) fn peer_pid(stream: &UnixStream) -> Option<libc::pid_t> {
    let mut pid: libc::pid_t = 0;
    let mut size = libc::socklen_t::try_from(size_of::<libc::pid_t>()).ok()?;
    // SAFETY: getsockopt writes at most `size` bytes, the peer's process id, into `pid`.
    let got = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&raw mut pid).cast(),
            &raw mut size,
        )
    };
    (got == 0 && pid > 0).then_some(pid)
}

/// The process at the other end of `stream`, when the system says.
#[cfg(target_os = "linux")]
pub(crate) fn peer_pid(stream: &UnixStream) -> Option<libc::pid_t> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut size = libc::socklen_t::try_from(size_of::<libc::ucred>()).ok()?;
    // SAFETY: getsockopt writes at most `size` bytes of the peer's credentials into `cred`.
    let got = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &raw mut size,
        )
    };
    (got == 0 && cred.pid > 0).then_some(cred.pid)
}

/// The process at the other end of `stream`: not told on this system.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn peer_pid(_stream: &UnixStream) -> Option<libc::pid_t> {
    None
}

/// This process's user id.
pub(crate) fn own_uid() -> u32 {
    // SAFETY: geteuid takes nothing and cannot fail.
    unsafe { libc::geteuid() }
}

/// Whether the other end of `stream` is this user's own process.
pub(crate) fn is_own(stream: &UnixStream) -> bool {
    peer_uid(stream) == Some(own_uid())
}

#[cfg(test)]
#[path = "peer_tests.rs"]
mod tests;
