//! The desk's side of the ledger (ledger.rs): a pane owned is a row of its project's, a pane gone
//! is closed, what a page showed and a terminal's title are kept; and the window is told each
//! project's past as it changes, for its history menu and its address field.

use std::path::Path;

use super::{Call, Desk};
use crate::browser::{Ledger, Past, TabKind, Visited, Went};
use crate::ledger::{self, Kind, Opened, Row};
use crate::projects::{self, ProjectId};

/// How many of a project's panes the history menu lists, and of its pages the address field
/// suggests.
const LISTED: usize = 50;

/// How often a terminal's title is written to the ledger at most: a program that animates its
/// title says it many times a second.
const TITLE_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// What a terminal's title writes have come to: when the last was done, and the latest title
/// since, kept for the next write or for when the row closes.
pub(super) struct Titled {
    at: std::time::Instant,
    pending: Option<String>,
}

/// What is known of a pane as it opens.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Facts<'a> {
    pub(super) title: &'a str,
    pub(super) url: Option<&'a str>,
    pub(super) cwd: Option<&'a Path>,
    pub(super) command: Option<&'a str>,
}

impl Desk {
    /// Pane `key` is `call`'s now, a `kind` with `facts`: owned, and a row of its project's
    /// ledger.
    pub(super) fn own(&self, key: &str, call: &Call, kind: Kind, facts: Facts<'_>) {
        crate::lock(&self.registry).own(key, call.owner, call.person);
        // A page's first load may have made its row before the call that opened it got here
        // (`went`): of the same project it is this tab's row; of another, it never was.
        let before = crate::lock(&self.ledger).get(key).copied();
        if let Some((row, project)) = before {
            if project == call.project.id && call.again.is_none_or(|again| again == row) {
                return;
            }
            crate::lock(&self.ledger).remove(key);
            let now = crate::board::now();
            let forgot = self.feed.write(|write| {
                Self::unplace_pane(write, row, now)?;
                ledger::forget(write.tx(), row)
            });
            if let Err(why) = forgot {
                log::warn!("tab {key}'s stray row stays: {why}");
            }
            self.tell_ledger(project);
        }
        let opened = Opened {
            project: call.project.id,
            kind,
            agent: call.agent,
            title: facts.title,
            url: facts.url,
            cwd: facts.cwd,
            command: facts.command,
        };
        let window = call.window.or_else(|| self.in_use());
        self.keep(key, &opened, (call.again, window));
    }

    /// A row for pane `key`, `opened`: a new one, or the row `again` names (a pane the run
    /// before left open). Placed in a tab in the same change: a tab of its own when it is new.
    fn keep(
        &self,
        key: &str,
        opened: &Opened<'_>,
        (again, window): (Option<Row>, Option<crate::layout::WindowId>),
    ) {
        if again.is_none() {
            self.begin(opened.project);
        }
        let now = crate::board::now();
        let kept = self.feed.write(|write| {
            let row = match again {
                Some(row) => {
                    ledger::reopen(write.tx(), row)?;
                    row
                }
                None => ledger::open(write.tx(), opened, now)?,
            };
            Self::place_pane(write, row, (opened.project, window), now)?;
            Ok(row)
        });
        match kept {
            Ok(row) => {
                crate::lock(&self.ledger).insert(key.to_string(), (row, opened.project));
                // The pane has its key now: the window draws it in its tab.
                let pane = crate::feed::Entity::Pane {
                    owner: self.owner_of(key),
                    key: key.to_string(),
                };
                self.feed.record(&pane, crate::feed::Op::Upserted);
                self.tell_ledger(opened.project);
            }
            Err(why) => log::warn!("tab {key} is not in the ledger: {why}"),
        }
    }

    /// The first pane of `project` this run: what the last run left open there (and was not
    /// opened again) closes, into its history. It never comes back after the person closes all
    /// of this run's: a restore reads what it had left before it opens the first one.
    fn begin(&self, project: ProjectId) {
        if !crate::lock(&self.begun).insert(project) {
            return;
        }
        let now = crate::board::now();
        let closed = self.feed.write(|write| {
            for tab in ledger::left_open(write.tx(), project)? {
                ledger::close(write.tx(), tab.row, now)?;
                Self::unplace_pane(write, tab.row, now)?;
            }
            Ok(())
        });
        if let Err(why) = closed {
            log::warn!("what the last run left open is not closed: {why}");
        }
    }

    /// Pane `key` closed: its row closed (a terminal's screen kept), its project's past told
    /// again.
    pub(super) fn unkeep(&self, key: &str) {
        self.flush_title(key);
        self.keep_screen(key);
        let Some((row, project)) = crate::lock(&self.ledger).remove(key) else {
            return;
        };
        let now = crate::board::now();
        let closed = self.feed.write(|write| {
            ledger::close(write.tx(), row, now)?;
            Self::unplace_pane(write, row, now)
        });
        match closed {
            Ok(()) => self.tell_ledger(project),
            Err(why) => log::warn!("tab {key} did not close in the ledger: {why}"),
        }
    }

    /// Page `key` showed `went`: kept; a page the desk did not open (a popup) is a row of the
    /// project it is of, else of the project shown, from now.
    pub(super) fn went(&self, key: &str, went: &Went<'_>) {
        let known = crate::lock(&self.ledger).get(key).copied();
        let (row, project) = match known {
            Some(known) => known,
            None => {
                // What this was told is late when its tab has closed since (it was queued while
                // the tab closed): no row is made for a page that is gone.
                let gone = (self.browser.as_ref())
                    .is_some_and(|browser| !browser.pages().contains(&key.to_string()));
                let Some(project) = self.project_of_group(went.group).filter(|_| !gone) else {
                    return;
                };
                let opened = Opened {
                    project,
                    kind: Kind::Page,
                    agent: None,
                    title: went.title,
                    url: Some(went.url),
                    cwd: None,
                    command: None,
                };
                // A page the window opened itself (a popup) is of the window the person works in.
                self.keep(key, &opened, (None, self.in_use()));
                let kept = crate::lock(&self.ledger).get(key).copied();
                match kept {
                    Some(known) => known,
                    None => return,
                }
            }
        };
        let now = crate::board::now();
        let news = (self.feed.store()).write(|tx| ledger::went(tx, row, went.url, went.title, now));
        match news {
            Ok(true) => self.tell_ledger(project),
            Ok(false) => {}
            Err(why) => log::warn!("what tab {key} showed is not in the ledger: {why}"),
        }
    }

    /// Terminal `key`'s screen kept, for when it is restored.
    fn keep_screen(&self, key: &str) {
        let Some(id) = key.strip_prefix('t').and_then(|id| id.parse::<u32>().ok()) else {
            return;
        };
        let row = crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        let session = crate::lock(&self.sessions).get(&id).cloned();
        let (Some(row), Some(session)) = (row, session) else {
            return;
        };
        // Masked: a secret a call typed in is on the screen as typed, and no value outlives the run.
        let (text, now) = (self.vault.mask(&session.text()), crate::board::now());
        if let Err(why) = (self.feed.store()).write(|tx| ledger::keep_screen(tx, row, &text, now)) {
            log::warn!("terminal {key}'s screen was not kept: {why}");
        }
    }

    /// Every open terminal's screen kept: the app quits, and opens them again next time.
    pub(super) fn keep_screens(&self) {
        let keys: Vec<String> = crate::lock(&self.ledger).keys().cloned().collect();
        for key in keys {
            self.flush_title(&key);
            self.keep_screen(&key);
        }
    }

    /// Terminal `key` is titled `title` (its program says what it does): kept in the ledger, at
    /// most once a [`TITLE_EVERY`]; the latest title waits for the next write, or for its row to
    /// close. The window is told the past again when a write is news: its history menu lists the
    /// title of an open terminal from there.
    pub(super) fn retitled(&self, key: &str, title: &str) {
        let now = std::time::Instant::now();
        {
            let mut titled = crate::lock(&self.titled);
            if let Some(last) = titled.get_mut(key)
                && now.duration_since(last.at) < TITLE_EVERY
            {
                last.pending = Some(title.to_string());
                return;
            }
            titled.insert(
                key.to_string(),
                Titled {
                    at: now,
                    pending: None,
                },
            );
        }
        self.write_title(key, title);
    }

    /// The latest title of terminal `key` still waiting is written.
    fn flush_title(&self, key: &str) {
        let pending = (crate::lock(&self.titled).remove(key)).and_then(|titled| titled.pending);
        if let Some(title) = pending {
            self.write_title(key, &title);
        }
    }

    fn write_title(&self, key: &str, title: &str) {
        let Some((row, project)) = crate::lock(&self.ledger).get(key).copied() else {
            return;
        };
        match (self.feed.store()).write(|tx| ledger::titled(tx, row, title)) {
            Ok(true) => self.tell_ledger(project),
            Ok(false) => {}
            Err(why) => log::warn!("tab {key}'s title is not in the ledger: {why}"),
        }
    }

    /// The project the window keys `group`, else Home.
    fn project_of_group(&self, group: &str) -> Option<ProjectId> {
        let known = projects::of_key(group).and_then(|id| self.project_by_id(id));
        known
            .or_else(|| self.home_project().ok())
            .map(|project| project.id)
    }

    /// The window told project `project`'s past: its panes, the latest first, each open one with
    /// its key; and its pages.
    pub(super) fn tell_ledger(&self, project: ProjectId) {
        let Some(browser) = self.browser.as_ref() else {
            return;
        };
        let read = self.feed.store().read(|db| {
            Ok((
                ledger::past(db, project, LISTED)?,
                ledger::visits(db, project, LISTED)?,
            ))
        });
        let (tabs, visits) = match read {
            Ok(read) => read,
            Err(why) => return log::warn!("the ledger could not be read: {why}"),
        };
        let open = crate::lock(&self.ledger).clone();
        let key_of = |row: Row| {
            (open.iter())
                .find(|(_, (kept, _))| *kept == row)
                .map(|(key, _)| key.clone())
        };
        let tabs = (tabs.into_iter())
            .map(|tab| Past {
                row: tab.row,
                kind: match tab.kind {
                    Kind::Page => TabKind::Page,
                    Kind::Terminal => TabKind::Terminal,
                    Kind::Board => TabKind::Board,
                },
                detail: (tab.url.clone())
                    .or_else(|| tab.cwd.as_ref().map(|cwd| cwd.display().to_string()))
                    .unwrap_or_default(),
                key: key_of(tab.row).filter(|_| tab.closed.is_none()),
                title: tab.title,
            })
            .collect();
        let visited = (visits.into_iter())
            .map(|visit| Visited {
                title: visit.title,
                url: visit.url,
            })
            .collect();
        browser.ledger(&projects::key(project), Ledger { tabs, visited });
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
