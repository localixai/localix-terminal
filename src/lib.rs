//! llx-desk: a native window of browser pages and terminal sessions that an agent works in and a
//! person watches. Tabs on the system's WebView (WKWebView on macOS, WebKitGTK on Linux), each
//! page driven through a probe script; terminal sessions in a PTY, painted into tabs of the same
//! window; a bar and a side that list the tabs of one project at a time, kept as one layout of
//! windows, workspaces and tabs; on macOS a small preview in a screen corner. The app is the
//! server (`llx-desk open`, or the bundle; `llx-desk serve` is its launch out of sight, as a call
//! starts it): an agent calls it from its shell, `llx-desk window '<json>'`, over a unix socket,
//! and the app keeps each caller's tabs apart, within the project they work in.
//!
//! The layers, from the window up: [`browser`] (the window and its pages), [`terminal`]
//! (sessions), [`desk`] (the `window` actions over both, tabs kept per owner and project), [`board`]
//! (the project's cards), [`projects`], [`ledger`] and [`layout`] (what the desk keeps of a
//! project, its panes and its windows), [`store`] (the database under them), [`setup`] (what the
//! app puts in place for the person's tools), [`server`] and [`client`] (the socket), [`cli`].
//! The desk knows no model: an agent of any kind drives it through the CLI, and its integration
//! (a plugin) is its own.

pub mod board;
pub mod browser;
pub mod bundle;
pub mod catalog;
pub mod cli;
pub mod client;
pub mod desk;
pub mod editor;
pub(crate) mod fds;
pub mod feed;
pub mod fetch;
pub mod files;
pub mod follow;
#[cfg(feature = "test-server")]
pub mod harness;
pub mod kind;
pub mod layout;
pub mod ledger;
pub mod mcp;
pub mod offers;
pub mod out;
pub mod page;
pub mod paths;
pub mod projects;
pub mod secrets;
pub mod served;
pub mod server;
pub mod setup;
pub mod shots;
pub mod store;
pub mod terminal;
pub mod update;
pub mod view;
pub mod wire;

#[cfg(test)]
mod testing;

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// A tab as the window lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTab {
    pub id: String,
    /// What it is (`browser`, `terminal`), as the window names it.
    pub kind: String,
    pub title: String,
    /// What it holds beside its title (a page's address), if anything.
    pub detail: String,
    /// It fills the window now.
    pub shown: bool,
    /// The tab the browser acts on.
    pub acted_on: bool,
    /// What it was opened for (a project), as whoever holds the window names it.
    pub group: String,
}

/// Now, in seconds since the epoch: the one clock of the desk's records (notices, boards, offers).
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// A mutex's guard, also when a holder panicked: the data a panicked holder left is still the
/// best there is, and the window must go on.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A wait on `condvar` for at most `slice`, also after a panicked holder.
pub(crate) fn wait_timeout<'a, T>(
    condvar: &Condvar,
    guard: MutexGuard<'a, T>,
    slice: Duration,
) -> MutexGuard<'a, T> {
    condvar
        .wait_timeout(guard, slice)
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0
}

/// A short pause of `slice` before trying again, where there is no event to wait on (a server
/// still starting, an accept that fails until a descriptor frees, a child that has not ended).
#[expect(
    clippy::disallowed_methods,
    reason = "a backoff: there is no event to wait on"
)]
pub(crate) fn backoff(slice: Duration) {
    std::thread::sleep(slice);
}

/// The usual pause of a [`backoff`].
pub(crate) const BACKOFF: Duration = Duration::from_millis(50);

/// 32 hex characters (16 random bytes) from the system's random source; unguessable: the probe's
/// messages carry one, so a page cannot speak as the probe, and a served site's prefix is one, so
/// nobody else on the machine can name it.
pub(crate) fn token() -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(32);
    for byte in random_bytes::<16>() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// `N` bytes from the system's cryptographic random source: the crate's one way to them.
pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    fill_random(&mut bytes);
    bytes
}

/// `bytes` filled by the kernel's generator, which never fails here.
#[cfg(target_vendor = "apple")]
fn fill_random(bytes: &mut [u8]) {
    // SAFETY: the buffer is valid for writes of its whole length.
    unsafe { libc::arc4random_buf(bytes.as_mut_ptr().cast(), bytes.len()) };
}

/// `bytes` filled by the kernel's generator, asked again when a signal cuts it short. Should it
/// ever refuse (a kernel without the call), the bytes come from std's own seeding, itself taken
/// from the system: weaker, never a panic.
#[cfg(not(target_vendor = "apple"))]
fn fill_random(bytes: &mut [u8]) {
    let mut filled = 0;
    while let Some(rest) = bytes.get_mut(filled..).filter(|rest| !rest.is_empty()) {
        // SAFETY: the rest of the buffer is valid for writes of its length.
        let got = unsafe { libc::getrandom(rest.as_mut_ptr().cast(), rest.len(), 0) };
        match usize::try_from(got) {
            Ok(got) => filled += got,
            Err(_) if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted => {
            }
            Err(_) => return seeded(rest),
        }
    }
}

#[cfg(not(target_vendor = "apple"))]
fn seeded(bytes: &mut [u8]) {
    use std::hash::{BuildHasher, Hasher};
    for chunk in bytes.chunks_mut(8) {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_usize(chunk.as_ptr() as usize);
        for (to, from) in chunk.iter_mut().zip(hasher.finish().to_le_bytes()) {
            *to = from;
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
