//! The person's comments on the pages (probe/feedback.js), held until they send them: each card
//! the person fills adds one here, and the strip's Send hands the whole batch to whoever listens,
//! one message per tab.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tao::event_loop::EventLoopProxy;

use super::capture::Shot;
use super::host::{Cmd, Noted, Shared};
use super::strip::Key;

/// Comments held at most: past it the oldest go.
const MAX: usize = 50;

#[derive(Default)]
pub(super) struct Comments {
    /// Each comment in words, with the tab it is on (`w1`).
    held: Mutex<Vec<(String, String)>>,
}

impl Comments {
    pub(super) fn add(&self, tab: &str, said: String) {
        let mut held = crate::lock(&self.held);
        held.push((tab.to_string(), said));
        let over = held.len().saturating_sub(MAX);
        held.drain(..over);
    }

    pub(super) fn count(&self) -> usize {
        crate::lock(&self.held).len()
    }

    /// Every comment held, as one message per tab in the order they came; none are held after.
    pub(super) fn take(&self) -> Vec<(String, String)> {
        let held = std::mem::take(&mut *crate::lock(&self.held));
        let mut batches: Vec<(String, Vec<String>)> = vec![];
        for (tab, said) in held {
            match batches.iter_mut().find(|(known, _)| *known == tab) {
                Some((_, saids)) => saids.push(said),
                None => batches.push((tab, vec![said])),
            }
        }
        batches
            .into_iter()
            .map(|(tab, saids)| {
                let head = format!("The person sent {} comment(s) on this page:", saids.len());
                (tab, [vec![head], saids].concat().join("\n\n"))
            })
            .collect()
    }
}

/// How long a comment's picture may take before the comment is held without it.
const PICTURE: Duration = Duration::from_secs(5);

/// Hold `noted` once its picture (`shot`, from the main thread) is in a file it names, or without
/// one: on a thread of its own, so the window never waits on the compositor. The strip is told.
pub(super) fn pictured(
    noted: Noted,
    shot: Receiver<Result<Shot, String>>,
    shared: &Arc<Shared>,
    proxy: &EventLoopProxy<Cmd>,
) {
    let (shared, proxy) = (Arc::clone(shared), proxy.clone());
    let hold = move || {
        let mut said = noted.said;
        let file = shot
            .recv_timeout(PICTURE)
            .ok()
            .and_then(Result::ok)
            .and_then(|shot| saved(&shot));
        if let Some(file) = file {
            said = format!("{said}\nPicture of it: {}", file.display());
        }
        shared.comments.add(&Key::Web(noted.tab).name(), said);
        let _ = proxy.send_event(Cmd::Strip);
    };
    // Ends within `PICTURE`: nothing has to stop it.
    let thread = std::thread::Builder::new().name("desk-comment".into());
    if let Err(error) = thread.spawn(hold) {
        super::debug(|| format!("a comment was lost: {error}"));
    }
}

/// Where the pictures of comments are kept: in the desk's home, which is the person's alone (a
/// shared temporary directory would let another user read, or plant, what the person pointed at).
fn directory() -> PathBuf {
    crate::paths::home().join("comments")
}

/// `shot` in a file of its own in [`directory`], where an agent of the person's can read it; the
/// newest few are kept, as screenshots are.
fn saved(shot: &Shot) -> Option<PathBuf> {
    crate::shots::save(&directory(), &shot.jpeg).ok()
}

#[cfg(test)]
#[path = "comments_tests.rs"]
mod tests;
