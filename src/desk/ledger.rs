//! The desk's side of the ledger (ledger.rs): a pane owned is a row of its project's, a pane gone
//! is closed, what a page showed and a terminal's title are kept. Each change of a project's past
//! is a notice of its ledger (`Entity::Ledger`), with which the window's consumer
//! (layout_drawn.rs) tells the window that past again, for its history menu and its address field.

use std::path::Path;

use super::{Call, Desk};
use crate::browser::Went;
use crate::feed::{Entity, Op};
use crate::ledger::{self, Kind, Opened, Row};
use crate::projects::{self, ProjectId};

/// How often a terminal's title is written to the ledger at most: a program that animates its
/// title says it many times a second.
const TITLE_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// What a terminal's title writes have come to: when the last was done, and the latest title
/// since, kept for the next write or for when the row closes.
pub(super) struct Titled {
    at: std::time::Instant,
    pending: Option<String>,
}

/// What a closing pane's row is closed with, taken from memory as it closes.
pub(super) struct Closing {
    row: Row,
    project: ProjectId,
    /// The latest title not written yet.
    title: Option<String>,
    /// A terminal's session, whose screen is kept.
    session: Option<std::sync::Arc<crate::terminal::Session>>,
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
                ledger::forget(write.tx(), row)?;
                write.record(&Entity::Ledger { project }, Op::Upserted)
            });
            if let Err(why) = forgot {
                log::warn!("tab {key}'s stray row stays: {why}");
            }
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
                // The pane has its key now: the window draws it in its tab, and its project's
                // past lists it by that key.
                let pane = Entity::Pane {
                    owner: self.owner_of(key),
                    key: key.to_string(),
                };
                self.feed.record(&pane, Op::Upserted);
                let project = opened.project;
                self.feed.record(&Entity::Ledger { project }, Op::Upserted);
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
            write.record(&Entity::Ledger { project }, Op::Upserted)
        });
        if let Err(why) = closed {
            log::warn!("what the last run left open is not closed: {why}");
        }
    }

    /// Pane `key` closed: its row closed (a terminal's screen kept), its project's past told
    /// again.
    pub(super) fn unkeep(&self, key: &str) {
        if let Some(closing) = self.closing(key) {
            self.close_row(key, &closing);
        }
    }

    /// Pane `key` let go of in memory as it closes: what its row's close writes, taken now, so
    /// the write may come later (`close_row`). None when it had no row.
    pub(super) fn closing(&self, key: &str) -> Option<Closing> {
        let title = (crate::lock(&self.titled).remove(key)).and_then(|titled| titled.pending);
        let session = self.session_of(key);
        let (row, project) = crate::lock(&self.ledger).remove(key)?;
        Some(Closing {
            row,
            project,
            title,
            session,
        })
    }

    /// The row of pane `key` closed as `closing` says: its last title and screen kept, out of its
    /// tab, its project's past told again.
    pub(super) fn close_row(&self, key: &str, closing: &Closing) {
        let Closing { row, project, .. } = *closing;
        if let Some(title) = &closing.title
            && let Err(why) = (self.feed.store()).write(|tx| ledger::titled(tx, row, title))
        {
            log::warn!("tab {key}'s title is not in the ledger: {why}");
        }
        if let Some(session) = &closing.session {
            self.keep_screen_of(key, row, session);
        }
        let now = crate::board::now();
        let closed = self.feed.write(|write| {
            ledger::close(write.tx(), row, now)?;
            write.record(&Entity::Ledger { project }, Op::Upserted)?;
            Self::unplace_pane(write, row, now)
        });
        if let Err(why) = closed {
            log::warn!("tab {key} did not close in the ledger: {why}");
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
        let news = self.feed.write(|write| {
            ledger::went(write.tx(), row, went.url, went.title, now)
                .and_then(|news| Self::news_of(write, project, news))
        });
        if let Err(why) = news {
            log::warn!("what tab {key} showed is not in the ledger: {why}");
        }
    }

    /// Terminal `key`'s screen kept, for when it is restored.
    fn keep_screen(&self, key: &str) {
        let row = crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        if let (Some(row), Some(session)) = (row, self.session_of(key)) {
            self.keep_screen_of(key, row, &session);
        }
    }

    /// The session of terminal `key`, while it has one.
    fn session_of(&self, key: &str) -> Option<std::sync::Arc<crate::terminal::Session>> {
        let id = key
            .strip_prefix('t')
            .and_then(|id| id.parse::<u32>().ok())?;
        crate::lock(&self.sessions).get(&id).cloned()
    }

    /// Terminal `key`'s screen, as `session` shows it now, kept in row `row`.
    fn keep_screen_of(&self, key: &str, row: Row, session: &crate::terminal::Session) {
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
        let titled = self.feed.write(|write| {
            ledger::titled(write.tx(), row, title)
                .and_then(|news| Self::news_of(write, project, news))
        });
        if let Err(why) = titled {
            log::warn!("tab {key}'s title is not in the ledger: {why}");
        }
    }

    /// A write of `project`'s ledger that was `news` (it changed what its past lists) is a notice
    /// of it, in the write's transaction; one that changed nothing is none.
    fn news_of(
        write: &crate::feed::Write<'_, '_>,
        project: ProjectId,
        news: bool,
    ) -> Result<(), String> {
        match news {
            true => write.record(&Entity::Ledger { project }, Op::Upserted),
            false => Ok(()),
        }
    }

    /// The project the window keys `group`, else Home.
    fn project_of_group(&self, group: &str) -> Option<ProjectId> {
        let known = projects::of_key(group).and_then(|id| self.project_by_id(id));
        known
            .or_else(|| self.home_project().ok())
            .map(|project| project.id)
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
