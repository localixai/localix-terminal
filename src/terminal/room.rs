//! How many terminals the app holds at once, read from what each one costs on this machine.
//!
//! A terminal holds three descriptors in the app (its PTY's end, and a copy each for the thread
//! that reads it and the one that writes to it), two threads, and its screen and history in the
//! terminal core, drawn by a view of the window's own that keeps no picture while out of sight.
//! Measured on 2026-11-17 (the app's footprint, one window, each terminal with 10,000 rows of
//! history): 63 MB with one terminal, 128 MB with 8, 202 MB with 16 and 350 MB with 32, so about
//! 9.3 MB each; the core holds a history to 16 MiB however wide its rows, so a terminal is
//! counted at that. The open-file limit and the machine's memory each bound the count, and the
//! tighter of the two holds.

/// Descriptors one terminal holds in the app.
const FDS_EACH: u64 = 3;
/// The memory one terminal takes at most: its history full, at the core's limit on it.
const EACH: u64 = 16 << 20;
/// The fewest terminals the app holds, whatever the machine says: what it always held.
const FLOOR: usize = 16;

/// The most terminals the app holds at once on this machine.
pub fn most() -> usize {
    bound(crate::fds::limit(), memory())
}

/// The most terminals within `fds` (the soft limit on open descriptors), half of which is left
/// for the rest of the app (connections, pages, the database, files watched), and within
/// `memory` (the machine's, in bytes), a quarter of which they may take. Never under
/// [`FLOOR`]; a limit not known bounds nothing.
fn bound(fds: Option<u64>, memory: Option<u64>) -> usize {
    let by_fds = fds.map(|fds| fds / 2 / FDS_EACH);
    let by_memory = memory.map(|memory| memory / 4 / EACH);
    let tighter = match (by_fds, by_memory) {
        (Some(a), Some(b)) => a.min(b),
        (Some(one), None) | (None, Some(one)) => one,
        (None, None) => 0,
    };
    usize::try_from(tighter).unwrap_or(usize::MAX).max(FLOOR)
}

/// The machine's memory in bytes, as the system says it.
fn memory() -> Option<u64> {
    // SAFETY: sysconf reads a value of the system and touches no memory of ours.
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    // SAFETY: as above.
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let pages = u64::try_from(pages).ok()?;
    let size = u64::try_from(size).ok()?;
    pages.checked_mul(size)
}

#[cfg(test)]
#[path = "room_tests.rs"]
mod tests;
