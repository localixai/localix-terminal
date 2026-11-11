//! What is opened from a terminal of the window lands beside that terminal: an agent running in
//! it asks for a page or a terminal, and the tab is the two of them, the terminal on the left,
//! still the pane in use. The call is known by the terminal its program descends from, so
//! nothing is asked of the agent.
//!
//! There is one place beside a terminal. The next pane opened from it takes that place and its
//! width, and the one before becomes a tab of its own right after. The place is the agent's only
//! while the person leaves it so: a tab they put beside the terminal themselves is not replaced,
//! and a split they made whole stays whole till they put one of the agent's panes back.
//!
//! The links are of this run: a terminal's program does not outlive the app.

use std::collections::{HashMap, HashSet};

use super::actions::{Kind, parse};
use super::layout::told;
use super::{Call, Desk};
use crate::feed::{Entity, Write};
use crate::layout::{self, Tree};
use crate::ledger::Row;

/// Which pane was opened from which terminal, and where the person said no.
#[derive(Debug, Default)]
pub(super) struct Links {
    /// Each pane opened from a terminal, and that terminal.
    from: HashMap<Row, Row>,
    /// The terminals whose split the person made whole: nothing lands beside them.
    whole: HashSet<Row>,
}

impl Links {
    /// Pane `pane` was opened from terminal `anchor`.
    fn opened(&mut self, pane: Row, anchor: Row) {
        self.from.insert(pane, anchor);
    }

    /// Whether pane `pane` was opened from terminal `anchor`.
    fn of(&self, anchor: Row, pane: Row) -> bool {
        self.from.get(&pane) == Some(&anchor)
    }

    /// Pane `pane` closed: it is no one's, and no place.
    pub(super) fn forget(&mut self, pane: Row) {
        self.from.remove(&pane);
        self.from.retain(|_, anchor| *anchor != pane);
        self.whole.remove(&pane);
    }

    /// Whether nothing is kept: no pane was opened from a terminal still open.
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.from.is_empty() && self.whole.is_empty()
    }
}

/// Where a pane opened from terminal `anchor` lands, its tab being `tree`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Landing {
    /// On the terminal's right: the tab was the terminal alone.
    Beside,
    /// In the place of this pane, opened from the terminal before it.
    Instead(Row),
    /// In a tab of its own: the person arranged the terminal's tab, or made it whole.
    Apart,
}

fn landing(links: &Links, tree: &Tree, anchor: Row) -> Landing {
    match tree.panes().as_slice() {
        [only] if *only == anchor && !links.whole.contains(&anchor) => Landing::Beside,
        [one, other] | [other, one] if *one == anchor && links.of(anchor, *other) => {
            Landing::Instead(*other)
        }
        _ => Landing::Apart,
    }
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

    /// The terminal what `call` opens lands beside: the one it came from, when that is a
    /// terminal of its project and the caller is the project's own agent. A named agent is one
    /// of several the same terminal may run, and they would take the one place in turn; what
    /// the person opens, or the app opens again, has its own place. None where the person
    /// chose tabs in Settings.
    pub(super) fn anchor_of(&self, call: &Call) -> Option<Row> {
        if call.agent.is_some() || call.person || call.again.is_some() {
            return None;
        }
        if (self.browser.as_ref()).is_some_and(|browser| !browser.opens_beside()) {
            return None;
        }
        let key = call.from.filter(|key| key.starts_with('t'))?;
        let (row, project) = crate::lock(&self.ledger).get(key).copied()?;
        (project == call.project.id).then_some(row)
    }

    /// The terminal a pane `call` opens now would land beside, as the layout is: what the window
    /// is told before the pane exists, so the new tab does not take the terminal's place.
    pub(super) fn lands_beside(&self, call: &Call) -> Option<Row> {
        let anchor = self.anchor_of(call)?;
        let wanted = call.window.or_else(|| self.in_use());
        let tree = (self.feed.store())
            .read(|db| {
                let Some(tab) = layout::tab_of(db, anchor)? else {
                    return Ok(None);
                };
                let Some(tab) = layout::tab(db, tab)? else {
                    return Ok(None);
                };
                // A pane opens in the window in use: a terminal of another window is no place.
                let window = layout::workspace(db, tab.workspace)?.map(|space| space.window);
                Ok((wanted.is_none() || wanted == window).then_some(tab.tree))
            })
            .ok()??;
        (landing(&crate::lock(&self.links), &tree, anchor) != Landing::Apart).then_some(anchor)
    }

    /// Pane `pane` put beside terminal `anchor` in its tab, in the same change as what opened
    /// it: on its right, or in the place of the pane opened from it before, which becomes a tab
    /// right after. The tab's panes as they were, or none when the pane lands apart (and nothing
    /// is changed).
    pub(super) fn place_beside(
        &self,
        write: &Write<'_, '_>,
        pane: Row,
        anchor: Row,
        now: u64,
    ) -> Result<Option<Vec<Row>>, String> {
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
        if pane == anchor {
            return Ok(None);
        }
        if before.contains(&pane) {
            return Ok(Some(before));
        }
        let landing = landing(&crate::lock(&self.links), &tab.tree, anchor);
        if landing == Landing::Apart {
            // Still that terminal's: the person putting it beside gives the place back.
            crate::lock(&self.links).opened(pane, anchor);
            return Ok(None);
        }
        // Out of the tab it was in (one shown again), which may close and move the others up.
        Self::unplace_pane(write, pane, now)?;
        let Some(tab) = tab_of(anchor)? else {
            return Ok(None);
        };
        let mut tree = tab.tree.clone();
        let mut focused = tab.focused.or(Some(anchor));
        match landing {
            Landing::Instead(old) if tree.replace(old, pane) => {
                if focused == Some(old) {
                    focused = Some(pane);
                }
                layout::set_tree(write.tx(), tab.id, &tree, focused)?;
                let after = Some(tab.place + 1);
                layout::open_tab(
                    write.tx(),
                    tab.workspace,
                    &Tree::leaf(old),
                    None,
                    after,
                    now,
                )?;
            }
            Landing::Beside | Landing::Instead(_) | Landing::Apart => {
                if !tree.split(anchor, pane, true, true) {
                    return Ok(None);
                }
                layout::set_tree(write.tx(), tab.id, &tree, focused)?;
            }
        }
        crate::lock(&self.links).opened(pane, anchor);
        let changed = [
            Entity::Tab { id: tab.id },
            Entity::Workspace { id: tab.workspace },
        ];
        told(write, &changed)?;
        Ok(Some(before))
    }

    /// Tab `key` shown as `call` asks. From a terminal it comes beside that terminal, which
    /// stays the pane in use while the window shows it; else the window shows the tab.
    pub(super) fn reveal(&self, call: &Call, key: &str) -> Result<(), String> {
        let row = crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        let beside = match (row, self.lands_beside(call)) {
            (Some(row), Some(anchor)) => {
                let now = crate::board::now();
                (self.feed)
                    .write(|write| self.place_beside(write, row, anchor, now))
                    .unwrap_or_else(|why| {
                        log::warn!("{key} is not shown beside its terminal: {why}");
                        None
                    })
            }
            _ => None,
        };
        if self.browser.is_none() && beside.is_none() {
            return Err("there is no window here to show it in".into());
        }
        if !self.bring(key, beside.as_deref()) {
            return Err(format!("the window has no tab {key} to show"));
        }
        // The layout's too: its tab the one its workspace shows, the pane in focus.
        if let (Some(row), None) = (row, &beside)
            && let Err(why) = self.show_pane(row)
        {
            log::warn!("{key} is not shown in the layout: {why}");
        }
        Ok(())
    }

    /// Tab `key` in sight. Put beside a terminal (`beside`: the panes that tab had), it is
    /// there already while the window shows one of them: the window lays the tab out as the
    /// layout says, and what it shows is not taken from the person. Else the window shows the
    /// tab. Whether it is in sight now (true where there is no window).
    pub(super) fn bring(&self, key: &str, beside: Option<&[Row]>) -> bool {
        let Some(browser) = self.browser.as_ref() else {
            return true;
        };
        if let Some(before) = beside {
            let seen = (browser.listing().unwrap_or_default().into_iter())
                .find(|tab| tab.shown)
                .and_then(|tab| crate::lock(&self.ledger).get(&tab.id).map(|(row, _)| *row));
            if seen.is_some_and(|seen| before.contains(&seen)) {
                return true;
            }
        }
        browser.act(key, false).is_ok()
    }

    /// The person split terminal `shown` and pane `beside` (either way round): with a pane
    /// opened from the terminal, the place beside it is that agent's again.
    pub(super) fn split_again(&self, shown: Row, beside: Row) {
        let mut links = crate::lock(&self.links);
        for (anchor, pane) in [(shown, beside), (beside, shown)] {
            if links.of(anchor, pane) {
                links.whole.remove(&anchor);
            }
        }
    }

    /// The person made the tab of `panes` whole: a terminal of it with its agent's pane beside
    /// stays whole from now. Only while both are open: a pane closing makes its tab whole too,
    /// and that is no one's wish.
    pub(super) fn made_whole(&self, panes: &[Row]) {
        let [one, other] = panes else {
            return;
        };
        if !self.is_open(*one) || !self.is_open(*other) {
            return;
        }
        let mut links = crate::lock(&self.links);
        for (anchor, pane) in [(*one, *other), (*other, *one)] {
            if links.of(anchor, pane) {
                links.whole.insert(anchor);
            }
        }
    }

    /// Whether pane `row` is open now: it has a key this run, and its page is still in the
    /// window, or its session still runs.
    fn is_open(&self, row: Row) -> bool {
        let key = (crate::lock(&self.ledger).iter())
            .find(|(_, (kept, _))| *kept == row)
            .map(|(key, _)| key.clone());
        let Some(key) = key else {
            return false;
        };
        match parse(&key).map(|tab| (tab.kind, tab.id)) {
            Some((Kind::Web, _)) => {
                (self.browser.as_ref()).is_none_or(|browser| browser.pages().contains(&key))
            }
            Some((Kind::Term, id)) => crate::lock(&self.sessions).contains_key(&id),
            _ => true,
        }
    }
}

#[cfg(test)]
#[path = "beside_tests.rs"]
mod tests;
