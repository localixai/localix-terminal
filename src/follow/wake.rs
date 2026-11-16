//! A process's end, waited on by a thread of its own that sleeps in the system until it happens
//! (kqueue's `NOTE_EXIT` on macOS, a pidfd on Linux), so a follower ends with its parent
//! without looking now and then. Where it cannot be set up, it fails, and the follower looks.

use std::io;

/// `then` called once process `pid` ends. An error when its end cannot be waited on (it is
/// gone already, or the system cannot say).
pub(super) fn on_exit(pid: libc::pid_t, then: impl FnOnce() + Send + 'static) -> io::Result<()> {
    let ending = system::Ending::of(pid)?;
    std::thread::Builder::new()
        .name("desk-follow-parent".into())
        .spawn(move || {
            if ending.wait().is_ok() {
                then();
            }
        })
        .map(drop)
}

#[cfg(target_os = "macos")]
#[path = "wake/kqueue.rs"]
mod system;

#[cfg(not(target_os = "macos"))]
#[path = "wake/pidfd.rs"]
mod system;

#[cfg(test)]
#[path = "wake_tests.rs"]
mod tests;
