//! The Store's sources as the person keeps them, and what follows from them: the list shown in
//! the page, a repository added or a source removed, the read of them that goes on while the
//! app runs, and what was installed kept up to date where the person left that on.

use std::collections::BTreeMap;
use std::sync::Weak;
use std::time::Duration;

use serde_json::{Value, json};

use super::Store;
use crate::catalog::sources::{self, Shape};

/// The sources are read a minute after the app starts, then about every hour: each answers
/// that nothing changed in one short request.
const FIRST_AFTER: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(61 * 60);

impl Store {
    /// The sources read from here on, on a thread of their own, so that the Store is full
    /// when it is opened and what is installed hears of a newer version.
    pub(super) fn watch(&self) {
        if cfg!(feature = "test-server") {
            return;
        }
        let me = Weak::clone(&self.me);
        let watching = std::thread::Builder::new()
            .name("store-watch".into())
            .spawn(move || {
                // Nothing is ever sent: the wait is the deadline, and ends with the process.
                let (_held, wait) = std::sync::mpsc::sync_channel::<()>(0);
                let mut after = FIRST_AFTER;
                while wait.recv_timeout(after).is_err() {
                    let Some(store) = me.upgrade() else { return };
                    store.read_sources(false);
                    after = EVERY;
                }
            });
        if let Err(why) = watching {
            log::warn!("the Store's sources are read only when it is open (no thread: {why})");
        }
    }

    /// The sources as the page lists them: what each is, how much it lists, when it was read.
    pub(super) fn sources(&self) -> Value {
        let held = self.index.sources();
        let now = crate::now();
        let state = crate::lock(&self.state);
        let rows: Vec<Value> = (state.sources.list().iter())
            .map(|kept| {
                let held = held.iter().find(|held| held.source == kept.entry.id);
                let holds = match kept.entry.shape {
                    Shape::Claude | Shape::Codex => "Plugins",
                    Shape::Skills => "Skills",
                    Shape::Registry => "MCP servers",
                };
                json!({
                    "id": kept.entry.id, "title": kept.source.title(), "repo": kept.entry.repo,
                    "holds": holds, "trust": kept.source.trust(),
                    "stock": kept.entry.stock,
                    "listings": held.map_or(0, |held| held.listings),
                    "ago": held.map(|held| now.saturating_sub(held.read_at)),
                    "problem": state.failed.get(&kept.entry.id),
                })
            })
            .collect();
        json!({"sources": rows, "removed": state.sources.removed()})
    }

    /// The repository the person named joins the sources, when it holds something to list.
    pub(super) fn add_source(&self, text: &str) -> Result<(), String> {
        let repo = sources::repo_of(text)
            .ok_or("A source is a GitHub repository: owner/repo, or its address")?;
        {
            let mut state = crate::lock(&self.state);
            if state.sources.of_repo(&repo).is_some() {
                return Err(format!("{repo} is a source already"));
            }
            state.said = format!("Reading {repo}…");
        }
        self.tell("begun", &self.begun(false));
        self.go("store-source", move |store| store.add_now(&repo));
        Ok(())
    }

    fn add_now(&self, repo: &str) {
        let mut more = false;
        let mut added = false;
        for (shape, source) in sources::ways(repo) {
            let cache = self.cache.join(source.id());
            let Ok(Some(batch)) = source.read(&*self.net, None, &cache) else {
                continue;
            };
            if batch.listings.is_empty() {
                continue;
            }
            let put = (self.index).put(source.id(), source.trust(), &batch, crate::now());
            if let Err(why) = put {
                log::warn!("the Store could not keep what {repo} lists: {why}");
                continue;
            }
            crate::lock(&self.state).sources.add(repo, shape, source);
            more = batch.more;
            added = true;
            break;
        }
        crate::lock(&self.state).said = match added {
            true => String::new(),
            false => format!("{repo} has no plugin marketplace and no skills to read"),
        };
        self.note(match added {
            true => format!("The person added the source {repo}."),
            false => format!("{repo} was not added as a source: nothing to list was found."),
        });
        self.tell("begun", &self.begun(added));
        self.tell("sources", &self.sources());
        if more {
            self.read_sources(false);
        }
    }

    /// Source `id` leaves the Store with what it lists. What was installed from it stays in
    /// the agents.
    pub(super) fn remove_source(&self, id: &str) -> Result<(), String> {
        {
            let mut state = crate::lock(&self.state);
            if !state.sources.remove(id) {
                return Ok(());
            }
            state.failed.remove(id);
        }
        let forgot = self.index.forget(id);
        // What was fetched for it: its name is one the list held, never a path.
        let _ = std::fs::remove_dir_all(self.cache.join(id));
        self.tell("begun", &self.begun(true));
        self.tell("sources", &self.sources());
        forgot
    }

    /// The app's own sources the person removed are read again.
    pub(super) fn restore_sources(&self) {
        crate::lock(&self.state).sources.restore();
        self.tell("sources", &self.sources());
        self.read_sources(false);
    }

    /// What the Store installed and is listed newer now is installed again at that version,
    /// when the person asks for it. A listing that needs values of the person is left for
    /// them, and so is one an agent no longer has.
    pub(super) fn update(&self) {
        for (listing, behind) in self.noted() {
            if !behind || !listing.needs().is_empty() {
                continue;
            }
            let agents =
                (crate::lock(&self.state).installed.of(&listing.id).cloned()).unwrap_or_default();
            for (agent, had) in agents {
                if had.is_empty() || had == listing.version {
                    continue;
                }
                let Ok(target) = self.target(&agent) else {
                    continue;
                };
                if target.has(&listing, self.env()).is_none() {
                    crate::lock(&self.state)
                        .installed
                        .set(&listing.id, &agent, None);
                    continue;
                }
                self.change(listing.clone(), target, Some(BTreeMap::new()));
            }
        }
    }
}

#[cfg(test)]
#[path = "sourced_tests.rs"]
mod tests;
