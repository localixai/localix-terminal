//! The doctor: every integration looked at and put right by the app itself, with nobody asking.
//! It runs when the server starts, however it was started (the app opened, or a call that
//! started it), and again every hour while it runs: an agent installed since, an entry another
//! program rewrote, or one that names the app where it was before it moved is put right
//! without the app being opened again. What the person took out stays out (`declined`).

use std::sync::OnceLock;
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

/// How long the doctor waits between its rounds.
const EVERY: Duration = Duration::from_secs(60 * 60);

/// What starts a round before its hour: held for as long as the app runs, so the doctor's
/// thread goes on.
static SOONER: OnceLock<SyncSender<()>> = OnceLock::new();

/// One round: what LLX Desk 0.1.6 left under its name taken out (once), then the command, then
/// every agent's integration. What was done, in words.
pub fn doctor() -> Vec<String> {
    let mut said = {
        let _one = crate::lock(&super::BUSY);
        super::former::sweep()
    };
    said.extend(super::opened());
    said.extend(super::auto());
    said
}

/// What a round said (`now`) that the round before (`before`) did not: the log holds what
/// changed, not the same lines every hour.
pub(super) fn news(before: &[String], now: &[String]) -> Vec<String> {
    (now.iter())
        .filter(|line| !before.contains(line))
        .cloned()
        .collect()
}

/// The doctor from here on, on a thread of its own (it runs the person's programs and their
/// login shell): a round now and one every hour, what is new of each given to `note`. A test
/// build has no doctor, nor has an app run apart from the person's: the tools it would write
/// into are the person's, and would name a program they do not use.
pub fn keep_well(note: impl Fn(&[String]) + Send + 'static) -> std::io::Result<()> {
    if cfg!(feature = "test-server") || crate::paths::apart() {
        return Ok(());
    }
    let (sooner, asked) = sync_channel(1);
    if SOONER.set(sooner).is_err() {
        return Ok(());
    }
    std::thread::Builder::new()
        .name("desk-doctor".into())
        .spawn(move || {
            let mut before = vec![];
            let mut go_on = true;
            while go_on {
                let now = doctor();
                let new = news(&before, &now);
                if !new.is_empty() {
                    note(&new);
                }
                before = now;
                go_on = asked.recv_timeout(EVERY) != Err(RecvTimeoutError::Disconnected);
            }
        })
        .map(drop)
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
