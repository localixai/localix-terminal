//! The desk's side of the ledger (ledger.rs): a tab owned is a row of its project's, a tab gone
//! is closed, what a page showed and a terminal's title are kept; and the window is told each
//! project's past as it changes, for its history menu and its address field.

use std::path::Path;

use super::{Call, Desk};
use crate::browser::{Ledger, Past, TabKind, Visited, Went};
use crate::ledger::{self, Kind, Opened, Row};
use crate::projects::{self, ProjectId};

/// How many of a project's tabs the history menu lists, and of its pages the address field
/// suggests.
const LISTED: usize = 50;

/// What is known of a tab as it opens.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Facts<'a> {
    pub(super) title: &'a str,
    pub(super) url: Option<&'a str>,
    pub(super) cwd: Option<&'a Path>,
    pub(super) command: Option<&'a str>,
}

impl Desk {
    /// Tab `key` is `call`'s now, a `kind` with `facts`: owned, and a row of its project's
    /// ledger.
    pub(super) fn own(&self, key: &str, call: &Call, kind: Kind, facts: Facts<'_>) {
        crate::lock(&self.registry).own(key, call.owner);
        // A page's first load may have made its row before the call that opened it got here
        // (`went`): of the same project it is this tab's row; of another, it never was.
        let before = crate::lock(&self.ledger).get(key).copied();
        if let Some((row, project)) = before {
            if project == call.project.id {
                return;
            }
            crate::lock(&self.ledger).remove(key);
            if let Err(why) = (self.feed.store()).write(|tx| ledger::forget(tx, row)) {
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
        self.keep(key, &opened);
    }

    /// A row for tab `key`, `opened`.
    fn keep(&self, key: &str, opened: &Opened<'_>) {
        self.begin(opened.project);
        let now = crate::board::now();
        match (self.feed.store()).write(|tx| ledger::open(tx, opened, now)) {
            Ok(row) => {
                crate::lock(&self.ledger).insert(key.to_string(), (row, opened.project));
                self.tell_ledger(opened.project);
            }
            Err(why) => log::warn!("tab {key} is not in the ledger: {why}"),
        }
    }

    /// The first tab of `project` this run: what the last run left open there (and was not
    /// opened again) closes, into its history. It never comes back after the person closes all
    /// of this run's: a restore reads what it had left before it opens the first one.
    fn begin(&self, project: ProjectId) {
        if !crate::lock(&self.begun).insert(project) {
            return;
        }
        let now = crate::board::now();
        let closed = (self.feed.store()).write(|tx| {
            for tab in ledger::left_open(tx, project)? {
                ledger::close(tx, tab.row, now)?;
            }
            Ok(())
        });
        if let Err(why) = closed {
            log::warn!("what the last run left open is not closed: {why}");
        }
    }

    /// Tab `key` closed: its row closed (a terminal's screen kept), its project's past told
    /// again.
    pub(super) fn unkeep(&self, key: &str) {
        self.keep_screen(key);
        let Some((row, project)) = crate::lock(&self.ledger).remove(key) else {
            return;
        };
        let now = crate::board::now();
        match (self.feed.store()).write(|tx| ledger::close(tx, row, now)) {
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
                let Some(project) = self.project_of_group(went.group) else {
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
                self.keep(key, &opened);
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
        let (text, now) = (session.text(), crate::board::now());
        if let Err(why) = (self.feed.store()).write(|tx| ledger::keep_screen(tx, row, &text, now)) {
            log::warn!("terminal {key}'s screen was not kept: {why}");
        }
    }

    /// Every open terminal's screen kept: the app quits, and opens them again next time.
    pub(super) fn keep_screens(&self) {
        let keys: Vec<String> = crate::lock(&self.ledger).keys().cloned().collect();
        for key in keys {
            self.keep_screen(&key);
        }
    }

    /// Terminal `key` is titled `title` (its program says what it does): kept.
    pub(super) fn retitled(&self, key: &str, title: &str) {
        let Some((row, project)) = crate::lock(&self.ledger).get(key).copied() else {
            return;
        };
        match (self.feed.store()).write(|tx| ledger::titled(tx, row, title)) {
            Ok(true) => self.tell_ledger(project),
            Ok(false) => {}
            Err(why) => log::warn!("tab {key}'s title is not in the ledger: {why}"),
        }
    }

    /// Tab `row` of the history opened again for the person, as `call` (of the project the
    /// window keys `group`).
    pub(super) fn again(&self, call: &Call, group: &str, row: Row) -> Result<(), String> {
        let tab = (self.feed.store().read(|db| ledger::get(db, row)))?
            .ok_or("that tab is no longer in the history")?;
        self.again_tab(call, group, &tab)
    }

    /// The person went to `call`'s project, which has no tab open: what it had open when the
    /// app last quit opened again, in its order; else their shell in its folder.
    pub(super) fn reopen_project(&self, call: &Call, group: &str) -> Result<(), String> {
        let open: std::collections::HashSet<Row> = (crate::lock(&self.ledger).values())
            .map(|(row, _)| *row)
            .collect();
        let left = (self.feed.store()).read(|db| ledger::left_open(db, call.project.id))?;
        let left: Vec<ledger::Tab> = (left.into_iter())
            .filter(|tab| !open.contains(&tab.row))
            .collect();
        if left.is_empty() {
            return self.person_terminal(call, group, "");
        }
        for tab in &left {
            if let Err(why) = self.again_tab(call, group, tab) {
                log::warn!("a tab of {} did not open again: {why}", call.project.name);
            }
        }
        Ok(())
    }

    /// `tab` opened again for the person: a page at its address, their shell in a terminal's
    /// folder (its program is not run again), the board. The new tab is the one the ledger keeps
    /// from then on: the row it was goes.
    fn again_tab(&self, call: &Call, group: &str, tab: &ledger::Tab) -> Result<(), String> {
        let opened = match tab.kind {
            Kind::Page => {
                let url = (tab.url.as_deref())
                    .filter(|url| !url.is_empty())
                    .unwrap_or(crate::browser::HOME);
                (self.open(call, &serde_json::json!({"kind": "browser", "url": url})))
                    .map(|_| ())
                    .map_err(|failure| failure.message().to_string())
            }
            Kind::Terminal => {
                let cwd = (tab.cwd.clone())
                    .filter(|cwd| cwd.is_dir())
                    .unwrap_or_else(|| call.cwd.to_path_buf());
                let before = (self.feed.store().read(|db| ledger::screen(db, tab.row)))
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                self.person_terminal_after(&Call { cwd: &cwd, ..*call }, group, "", &before)
            }
            Kind::Board => (self.board(call, &serde_json::json!({"op": "show"})))
                .map(|_| ())
                .map_err(|failure| failure.message().to_string()),
        };
        opened?;
        (self.feed.store()).write(|tx| ledger::forget(tx, tab.row))?;
        self.tell_ledger(tab.project);
        Ok(())
    }

    /// The project the window keys `group`, else Home.
    fn project_of_group(&self, group: &str) -> Option<ProjectId> {
        let known = projects::of_key(group).and_then(|id| self.project_by_id(id));
        known
            .or_else(|| self.home_project().ok())
            .map(|project| project.id)
    }

    /// The window told project `project`'s past: its tabs, the latest first, each open one with
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
