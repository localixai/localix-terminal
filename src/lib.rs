//! llx-desk: a native window of browser pages and terminal sessions that an agent works in and a
//! person watches. Tabs on the system's WebView (WKWebView on macOS, WebKitGTK on Linux), each
//! page driven through a probe script; terminal sessions in a PTY, painted into tabs of the same
//! window; a strip of tabs along the top, grouped by project; on macOS a small preview in a
//! screen corner. An agent calls it from its shell: `llx-desk window '<json>'` reaches the app
//! (`llx-desk serve`) over a unix socket, and the app keeps each caller's tabs apart.
//!
//! The layers, from the window up: [`browser`] (the window and its pages), [`terminal`]
//! (sessions), [`desk`] (the `window` actions over both, tabs kept per owner), [`board`] (the
//! project's cards), [`server`] and [`client`] (the socket), [`cli`]. The desk knows no model: an
//! agent of any kind drives it through the CLI, and its integration (a plugin) is its own.

pub mod board;
pub mod browser;
pub mod bundle;
pub mod cli;
pub mod client;
pub mod desk;
pub mod feed;
pub mod follow;
#[cfg(feature = "test-server")]
pub mod harness;
pub mod ledger;
pub mod mcp;
pub mod offers;
pub mod out;
pub mod paths;
pub mod projects;
pub mod secrets;
pub mod served;
pub mod server;
pub mod setup;
pub mod shots;
pub mod store;
pub mod terminal;
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

/// 32 hex characters from the clock and the address space; unguessable to a page in one run: the
/// probe's messages carry it, so a page cannot speak as the probe.
pub(crate) fn token() -> String {
    use std::collections::hash_map::RandomState;
    use std::fmt::Write as _;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for _ in 0..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        let _ = write!(out, "{:016x}", hasher.finish());
    }
    out
}
