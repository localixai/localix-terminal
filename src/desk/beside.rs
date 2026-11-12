//! A session and its documents (docs/windows.md): a terminal's tab holds the files and pages
//! opened in it, a stack on the terminal's right with one of them in sight, so each session
//! keeps its own and the side lists the sessions alone. A document joins the session it was
//! asked from: an agent's, the terminal its program runs in (the call is known by that
//! terminal, so nothing is asked of the agent); the person's, the session they look at.
//!
//! An agent's document is the one in sight unless the person is in a document themselves:
//! then it waits behind, a tab over the stack. The terminal keeps the keys either way. What
//! the person opens is the pane in use, since they opened it to read.
//!
//! A document asked from no session joins one of its project all the same: the one shown,
//! else its last (sessions.rs); only a project with no terminal keeps documents in a tab of
//! their own, until its first session takes them. A session is always a tab of the caller's
//! own project, in the window it works in.

use super::layout::told;
use super::{Call, Desk};
use crate::feed::{Entity, Write};
use crate::layout::{self, Tree};
use crate::ledger::{self, Kind, Row};

/// The session a document joins: the tab of its pane `anchor`. `person`, the person opened the
/// document, and it is the pane in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Beside {
    pub(super) anchor: Row,
    pub(super) person: bool,
}

/// Whether a pane of `kind` is a document: what a session holds, and no tab of the side.
pub(super) fn document(kind: Kind) -> bool {
    matches!(kind, Kind::Page | Kind::File)
}

/// Whether pane `pane` is of a kind `wanted` takes.
pub(super) fn is(
    db: &crate::store::Connection,
    pane: Row,
    wanted: fn(Kind) -> bool,
) -> Result<bool, String> {
    Ok(ledger::get(db, pane)?.is_some_and(|pane| wanted(pane.kind)))
}

/// Whether a document may join the tab of `tree` by its pane `anchor`: it holds documents
/// already, or is a terminal alone. A board, or panes the person put side by side, hold none.
pub(super) fn takes(
    db: &crate::store::Connection,
    tree: &Tree,
    anchor: Row,
) -> Result<bool, String> {
    if !tree.stacked().is_empty() {
        return Ok(true);
    }
    Ok(*tree == Tree::leaf(anchor) && is(db, anchor, |kind| kind == Kind::Terminal)?)
}

impl Desk {
    /// The terminal process `pid` runs in: the one whose program it is, or descends from.
    pub fn pane_of(&self, pid: libc::pid_t) -> Option<String> {
        let programs: Vec<(libc::pid_t, u32)> = (crate::lock(&self.sessions).iter())
            .filter_map(|(id, session)| Some((libc::pid_t::try_from(session.pid()).ok()?, *id)))
            .collect();
        let pids: Vec<libc::pid_t> = programs.iter().map(|(pid, _)| *pid).collect();
        let found = crate::terminal::lineage::ancestor(pid, &pids)?;
        (programs.iter())
            .find(|(pid, _)| *pid == found)
            .map(|(_, id)| format!("t{id}"))
    }

    /// The session a document `call` opens was asked from, by one of its panes. An agent's: the
    /// terminal its call came from, when that is a terminal of its project. The person's: the
    /// pane their call names, else the one they look at (`seen_in`). What the app opens again
    /// in the tab it had stays there.
    pub(super) fn anchor_of(&self, call: &Call) -> Option<Beside> {
        if let Some(row) = call.again {
            let placed = (self.feed.store()).read(|db| layout::tab_of(db, row));
            if !call.person || placed.ok().flatten().is_some() {
                return None;
            }
        }
        // Only a pane of the caller's own project is a session of it.
        let own = |key: &str| {
            let (row, project) = crate::lock(&self.ledger).get(key).copied()?;
            (project == call.project.id).then_some(row)
        };
        if call.person {
            let anchor = match call.from {
                Some(key) => own(key)?,
                None => return self.seen_in(call.project.id, call.window),
            };
            return Some(Beside {
                anchor,
                person: true,
            });
        }
        let anchor = call.from.filter(|key| key.starts_with('t')).and_then(own)?;
        Some(Beside {
            anchor,
            person: false,
        })
    }

    /// The session the person looks at in `project`, by the pane in use in the window, as the
    /// window says (the layout hears of a tab shown only with what follows it); with no window
    /// here, or another project in it, by the pane in focus of the tab the layout shows in
    /// `window` (none: the one in use).
    pub(super) fn seen_in(
        &self,
        project: crate::projects::ProjectId,
        window: Option<layout::WindowId>,
    ) -> Option<Beside> {
        let person = |anchor: Row| Beside {
            anchor,
            person: true,
        };
        let shown = (self.browser.as_ref())
            .and_then(crate::browser::Browser::listing)
            .and_then(|tabs| tabs.into_iter().find(|tab| tab.shown))
            .and_then(|tab| crate::lock(&self.ledger).get(&tab.id).copied())
            .filter(|(_, of)| *of == project);
        if let Some((row, _)) = shown {
            return Some(person(row));
        }
        let wanted = window.or_else(|| self.in_use());
        let seen: Option<Row> = (self.feed.store())
            .read(|db| {
                let first = layout::windows(db)?.first().map(|window| window.id);
                let Some(window) = wanted.or(first) else {
                    return Ok(None);
                };
                let space = (layout::workspaces(db, window)?.into_iter())
                    .find(|space| space.project == project);
                let Some(tab) = space.and_then(|space| space.shown) else {
                    return Ok(None);
                };
                Ok(layout::tab(db, tab)?
                    .and_then(|tab| tab.focused.or_else(|| tab.tree.panes().first().copied())))
            })
            .ok()?;
        seen.map(person)
    }

    /// Whether what `call` opens leaves the window to what it shows: an agent's call from a
    /// terminal of its project, where the person may be typing.
    pub(super) fn quiet(&self, call: &Call) -> bool {
        let own = |key: &str| {
            (crate::lock(&self.ledger).get(key)).is_some_and(|(_, of)| *of == call.project.id)
        };
        !call.person && call.again.is_none() && call.from.is_some_and(own)
    }

    /// The session a new document of `call` joins, as the layout is: the one it was asked
    /// from, else one of its project all the same (sessions.rs). It is what the window is told
    /// before the pane exists, so the new tab does not take the place of the session it joins.
    pub(super) fn opens_in(&self, call: &Call) -> Option<Beside> {
        (self.lands_beside(call)).or_else(|| self.session_for(call))
    }

    /// The session `call` asks its document from, when that tab takes one as the layout is.
    pub(super) fn lands_beside(&self, call: &Call) -> Option<Beside> {
        let beside = self.anchor_of(call)?;
        let anchor = beside.anchor;
        let wanted = call.window.or_else(|| self.in_use());
        let taken = (self.feed.store())
            .read(|db| {
                let Some(tab) = layout::tab_of(db, anchor)? else {
                    return Ok(false);
                };
                let Some(tab) = layout::tab(db, tab)? else {
                    return Ok(false);
                };
                // A pane opens in the window in use: a session of another window is no place.
                let window = layout::workspace(db, tab.workspace)?.map(|space| space.window);
                Ok((wanted.is_none() || wanted == window) && takes(db, &tab.tree, anchor)?)
            })
            .ok()?;
        taken.then_some(beside)
    }

    /// Document `pane` put in the session of `beside`, in the same change as what opened it:
    /// the last of its documents, or shown among them when it is one already. The person's is
    /// the one in sight and in focus, in a tab no longer whole; an agent's is in sight unless
    /// the person is in a document of that session, and the focus stays. The tab's panes as
    /// they were, or none when the tab takes no document (and nothing is changed).
    pub(super) fn place_beside(
        write: &Write<'_, '_>,
        pane: Row,
        beside: Beside,
        now: u64,
    ) -> Result<Option<Vec<Row>>, String> {
        let Beside { anchor, person } = beside;
        let tab_of = |pane: Row| -> Result<Option<layout::Tab>, String> {
            match layout::tab_of(write.tx(), pane)? {
                Some(id) => layout::tab(write.tx(), id),
                None => Ok(None),
            }
        };
        let Some(tab) = tab_of(anchor)? else {
            return Ok(None);
        };
        let before = tab.tree.panes();
        if pane == anchor || !takes(write.tx(), &tab.tree, anchor)? {
            return Ok(None);
        }
        let stacked = tab.tree.stacked();
        // The document the person is in, which an agent's does not cover.
        let reading = tab.focused.filter(|focused| stacked.contains(focused));
        let mut tree = tab.tree.clone();
        let mut tab = tab;
        if before.contains(&pane) {
            if !stacked.contains(&pane) {
                return Ok(Some(before));
            }
        } else {
            // Out of the tab it was in (one shown again), which may close and move the others up.
            Self::unplace_pane(write, pane, now)?;
            let Some(again) = tab_of(anchor)? else {
                return Ok(None);
            };
            tab = again;
            tree = tab.tree.clone();
            if !tree.stack(pane) {
                return Ok(None);
            }
        }
        let focused = match (person, reading) {
            (true, _) => {
                tree.show(pane);
                Some(pane)
            }
            (false, Some(reading)) => {
                tree.show(reading);
                Some(reading)
            }
            (false, None) => {
                tree.show(pane);
                tab.focused.or(Some(anchor))
            }
        };
        if tree != tab.tree || focused != tab.focused {
            layout::set_tree(write.tx(), tab.id, &tree, focused)?;
        }
        if person {
            layout::set_whole(write.tx(), tab.id, false)?;
            layout::show_tab(write.tx(), tab.workspace, Some(tab.id))?;
        }
        let changed = [
            Entity::Tab { id: tab.id },
            Entity::Workspace { id: tab.workspace },
        ];
        told(write, &changed)?;
        Ok(Some(before))
    }

    /// Tab `key` shown as `call` asks. A document asked from a session joins it and is shown
    /// there: an agent's leaves the window to the person while they look at that session; else
    /// the window shows the tab.
    pub(super) fn reveal(&self, call: &Call, key: &str) -> Result<(), String> {
        let row = crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        let session = match key.starts_with(['w', 'f']) {
            true => self.lands_beside(call),
            false => None,
        };
        let joined = match (row, session) {
            (Some(row), Some(session)) => {
                let now = crate::board::now();
                (self.feed)
                    .write(|write| Self::place_beside(write, row, session, now))
                    .unwrap_or_else(|why| {
                        log::warn!("{key} is not shown in its session: {why}");
                        None
                    })
            }
            _ => None,
        };
        if self.browser.is_none() && joined.is_none() {
            return Err("there is no window here to show it in".into());
        }
        if !self.bring(key, joined.as_deref(), call.person) {
            return Err(format!("the window has no tab {key} to show"));
        }
        // The layout's too: its tab the one its workspace shows, the pane in focus.
        if let (Some(row), None) = (row, &joined)
            && let Err(why) = self.show_pane(row)
        {
            log::warn!("{key} is not shown in the layout: {why}");
        }
        Ok(())
    }

    /// Tab `key` in sight. An agent's document put in a session (`joined`: the panes that tab
    /// had) is there already while the window shows one of them: the window lays the tab out as
    /// the layout says, and what it shows is not taken from the person. Else, and for what the
    /// `person` opened, the window shows the tab. Whether it is in sight now (true where there
    /// is no window).
    ///
    /// The window is told the layout first, ahead of the feed (which tells it again, the same):
    /// a document shown before the window knows its session would take the whole window for a
    /// moment, then move to its half.
    pub(super) fn bring(&self, key: &str, joined: Option<&[Row]>, person: bool) -> bool {
        let Some(browser) = self.browser.as_ref() else {
            return true;
        };
        if joined.is_some() {
            self.tell_arranged();
        }
        if let (Some(before), false) = (joined, person) {
            let seen = (browser.listing().unwrap_or_default().into_iter())
                .find(|tab| tab.shown)
                .and_then(|tab| crate::lock(&self.ledger).get(&tab.id).map(|(row, _)| *row));
            if seen.is_some_and(|seen| before.contains(&seen)) {
                return true;
            }
        }
        browser.act(key, false).is_ok()
    }
}

#[cfg(test)]
#[path = "beside_tests.rs"]
mod tests;
