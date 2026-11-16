//! Splits and sessions (docs/windows.md): a tab takes several terminals and keeps one stack of
//! documents for all of them. A document told beside a terminal is shown in its session, or
//! joins the session it was put beside; a terminal alone joins a tab of terminals; a new
//! terminal asked beside a pane opens in that pane's tab, in the folder of the terminal beside;
//! and a pane taken out is a tab of its own, but for a session of one terminal, made whole: its
//! documents put by without letting go of them.

use std::path::PathBuf;

use super::beside::{document, is, takes};
use super::layout::told;
use super::{Call, Desk};
use crate::browser::Asked;
use crate::feed::{Entity, Write};
use crate::layout::geometry::Side;
use crate::layout::{self, Tree};
use crate::ledger::{self, Kind, Row};

impl Desk {
    /// The split of `shown` and `beside` (on `side` of `shown`) kept as a session keeps it,
    /// `shown` the pane in use. Whether it was one: of one tab, a document of its stack in
    /// sight and the tab no longer whole; of two, a document joining the session beside it. A
    /// terminal alone in its tab joins a tab of terminals as any split does (not one); a tab
    /// with documents takes nothing else, and does not leave them for another tab: refused.
    pub(super) fn split_session(
        write: &Write<'_, '_>,
        (shown, beside): (Row, Row),
        side: Side,
        now: u64,
    ) -> Result<bool, String> {
        let tx = write.tx();
        let tab_of = |pane: Row| -> Result<Option<layout::Tab>, String> {
            match layout::tab_of(tx, pane)? {
                Some(id) => layout::tab(tx, id),
                None => Ok(None),
            }
        };
        let (Some(mine), Some(other)) = (tab_of(shown)?, tab_of(beside)?) else {
            return Ok(false);
        };
        let left = if side.forward() { shown } else { beside };
        if mine.id == other.id {
            let stacked = mine.tree.stacked();
            let Some(doc) = [shown, beside]
                .into_iter()
                .find(|pane| stacked.contains(pane))
            else {
                return Ok(false);
            };
            let mut tree = mine.tree.clone();
            tree.show(&doc);
            // Two documents of one stack are never side by side: the one in use is in sight.
            if stacked.contains(&shown) {
                tree.show(&shown);
            }
            tree.order(&left);
            if tree != mine.tree || mine.focused != Some(shown) {
                layout::set_tree(tx, mine.id, &tree, Some(shown))?;
            }
            layout::set_whole(tx, mine.id, false)?;
            told(write, &[Entity::Tab { id: mine.id }])?;
            return Ok(true);
        }
        // Which of the two is the document, and which the session it joins.
        let joins = if is(tx, beside, document)? && takes(tx, &mine.tree, shown)? {
            Some((beside, shown))
        } else if is(tx, shown, document)? && takes(tx, &other.tree, beside)? {
            Some((shown, beside))
        } else {
            None
        };
        let Some((doc, session)) = joins else {
            let alone =
                other.tree == Tree::leaf(beside) && is(tx, beside, |kind| kind == Kind::Terminal)?;
            if alone && takes(tx, &mine.tree, shown)? {
                return Ok(false);
            }
            let kept = [&mine, &other]
                .iter()
                .any(|tab| !tab.tree.stacked().is_empty());
            return match kept {
                true => {
                    Err("a session keeps its documents, and takes no pane but a terminal".into())
                }
                false => Ok(false),
            };
        };
        Self::unplace_pane(write, doc, now)?;
        let Some(tab) = tab_of(session)? else {
            return Ok(false);
        };
        let mut tree = tab.tree.clone();
        if !tree.stack(doc) {
            return Ok(false);
        }
        tree.order(&left);
        layout::set_tree(tx, tab.id, &tree, Some(shown))?;
        layout::set_whole(tx, tab.id, false)?;
        layout::show_tab(tx, tab.workspace, Some(tab.id))?;
        told(
            write,
            &[
                Entity::Tab { id: tab.id },
                Entity::Workspace { id: tab.workspace },
            ],
        )?;
        Ok(true)
    }

    /// Pane `pane` taken out of the tab it shares: a tab of its own right after it (shown when
    /// it was the tab's pane in focus), the rest as it was without it. A session of one
    /// terminal, or a document of a session, is made whole instead: it shows `pane` alone, its
    /// documents put by and still its own.
    pub(super) fn pane_out(&self, pane: Row) -> Result<(), String> {
        let now = crate::board::now();
        self.feed.write(|write| {
            let tx = write.tx();
            let Some(id) = layout::tab_of(tx, pane)? else {
                return Ok(());
            };
            let Some(tab) = layout::tab(tx, id)? else {
                return Ok(());
            };
            let stacked = tab.tree.stacked();
            let own = tab.tree.panes().len() - stacked.len();
            if stacked.contains(&pane) || (own == 1 && !stacked.is_empty()) {
                let mut tree = tab.tree;
                tree.show(&pane);
                layout::set_tree(tx, id, &tree, Some(pane))?;
                layout::set_whole(tx, id, true)?;
                return told(write, &[Entity::Tab { id }]);
            }
            let Some(tree) = tab.tree.clone().without(&pane) else {
                return Ok(());
            };
            let focused = (tab.focused.filter(|kept| *kept != pane && tree.holds(kept)))
                .or_else(|| tree.visible().first().copied());
            layout::set_tree(tx, id, &tree, focused)?;
            layout::set_whole(tx, id, false)?;
            let at = Some(tab.place + 1);
            let out = layout::open_tab(tx, tab.workspace, &Tree::leaf(pane), Some(pane), at, now)?;
            if tab.focused == Some(pane) {
                layout::show_tab(tx, tab.workspace, Some(out))?;
            }
            told(
                write,
                &[Entity::Tab { id }, Entity::Workspace { id: tab.workspace }],
            )
        })
    }

    /// New pane `pane` on `side` of pane `beside`, in `beside`'s tab, the tab's zoom ended.
    /// The person's (`person`) is the pane in focus and its tab the one shown; an agent's leaves
    /// both as they were. The panes the tab had; none, and nothing changed, where `beside` is in
    /// no tab.
    pub(super) fn split_in(
        write: &Write<'_, '_>,
        pane: Row,
        (beside, side): (Row, Side),
        person: bool,
    ) -> Result<Option<Vec<Row>>, String> {
        let tx = write.tx();
        let Some(id) = layout::tab_of(tx, beside)? else {
            return Ok(None);
        };
        let Some(tab) = layout::tab(tx, id)? else {
            return Ok(None);
        };
        let before = tab.tree.panes();
        let mut tree = tab.tree;
        if !tree.split(&beside, pane, side.across(), side.forward()) {
            return Ok(None);
        }
        let focused = if person { Some(pane) } else { tab.focused };
        layout::set_tree(tx, id, &tree, focused)?;
        layout::set_whole(tx, id, false)?;
        let mut changed = vec![Entity::Tab { id }];
        if person {
            layout::show_tab(tx, tab.workspace, Some(id))?;
            changed.push(Entity::Workspace { id: tab.workspace });
        }
        told(write, &changed)?;
        Ok(Some(before))
    }

    /// A new terminal the person asked for on `side` of pane `beside` of their project
    /// (`asked`: their shell, or an agent's program), in its tab and taking the keys: in the
    /// folder of the terminal beside, else the project's. Its key; what is no terminal is
    /// opened as "+" opens it.
    pub(super) fn person_beside(
        &self,
        call: &Call<'_>,
        group: &str,
        (beside, side): (&str, Side),
        asked: &Asked,
    ) -> Result<String, String> {
        let command = match asked {
            Asked::Terminal(_) => "",
            Asked::Agent(command) => command,
            other @ (Asked::Page(_)
            | Asked::File(_)
            | Asked::Text(_)
            | Asked::Board
            | Asked::Kind(_)
            | Asked::Worker { .. }
            | Asked::Session { .. }
            | Asked::Again(_)
            | Asked::Project
            | Asked::Window) => {
                return self.person_does(call, group, other).map(|()| String::new());
            }
        };
        let found = crate::lock(&self.ledger).get(beside).copied();
        let Some((row, _)) = found.filter(|(_, project)| *project == call.project.id) else {
            return Err(format!("{beside} is no pane of the project"));
        };
        let folder = (self.folder_of(beside, row)).unwrap_or_else(|| call.project.path.clone());
        let call = Call {
            cwd: &folder,
            ..*call
        };
        self.terminal_placed(&call, group, (command, command, ""), Some((row, side)))
    }

    /// Where terminal `key` (row `row`) is: its shell's folder now (`foreground::cwd`), else
    /// the one it opened in. None for a pane of another kind, or a folder gone.
    fn folder_of(&self, key: &str, row: Row) -> Option<PathBuf> {
        if !key.starts_with('t') {
            return None;
        }
        let now = self.session_of(key).and_then(|session| session.folder());
        let kept = || {
            let pane = (self.feed.store()).read(|db| ledger::get(db, row)).ok()??;
            pane.cwd
        };
        now.or_else(kept).filter(|folder| folder.is_dir())
    }
}

#[cfg(test)]
#[path = "layout_split_tests.rs"]
mod tests;
