//! The app's limit on open descriptors: raised for the app, and kept as it was for what the app
//! starts (a terminal's program, a login shell), which was written for the limit it always had.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::OnceLock;

/// The most descriptors the app asks for: a connection takes two, a terminal a few, a WebView more.
const FDS: libc::rlim_t = 10_240;

/// The limit the app was started with, before [`raise`].
static STARTED_WITH: OnceLock<libc::rlimit> = OnceLock::new();

/// The soft limit on open descriptors raised toward `FDS` (never past the hard one): an app the
/// system launches starts with 256 on macOS, which a few dozen agents and terminals use up.
pub(crate) fn raise() {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit writes into `limit`, which outlives the call.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) } != 0 {
        return;
    }
    let _ = STARTED_WITH.set(limit);
    let wanted = limit.rlim_max.min(FDS);
    if limit.rlim_cur >= wanted {
        return;
    }
    limit.rlim_cur = wanted;
    // SAFETY: setrlimit reads `limit` only; a refusal leaves the limit as it was.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const limit) } != 0 {
        log::warn!(
            "the open-files limit stays {}: {}",
            limit.rlim_cur,
            io::Error::last_os_error()
        );
    }
}

/// `command` starts with the limit the app had when it was started, not the one it raised.
pub(crate) fn as_started(command: &mut Command) {
    let Some(&started) = STARTED_WITH.get() else {
        return;
    };
    // SAFETY: between fork and exec the closure calls only setrlimit, which is async-signal-safe,
    // and allocates nothing.
    unsafe { command.pre_exec(move || restore(&started)) };
}

/// The limit set to `started` (lowering the soft limit is always allowed).
fn restore(started: &libc::rlimit) -> io::Result<()> {
    // SAFETY: setrlimit reads `started` only.
    match unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, started) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}
