//! The layout as the window tool reaches it (docs/windows.md, Agents): `open` with `beside`
//! puts a new terminal on a side of a pane of the caller's project, or a document in that
//! pane's session, and leaves the person's focus and the tab they look at as they were; `list`
//! says of each pane which tab it is in and what lies beside it. An agent moves no divider and
//! no keys between panes: those are the person's. Its zoom is `show`'s (layout_zoom.rs).

use std::fmt::Write as _;

use serde_json::Value;

use crate::desk::actions::Kind;
use crate::desk::args::text;
use crate::desk::{Call, Desk};
use crate::layout::geometry::{self, Rect, Side, Spacing};
use crate::layout::{self, LEAST, Tree};
use crate::ledger::Row;

/// Where `open` puts a tab asked `beside` a pane: that pane's key and row, and the side.
#[derive(Debug, Clone)]
pub(in crate::desk) struct Placed {
    pub(in crate::desk) key: String,
    pub(in crate::desk) row: Row,
    pub(in crate::desk) side: Side,
}

/// How `list` lays a tab out to say what lies beside a pane: any area, panes a point apart.
const LISTED: Spacing = Spacing {
    gap: 1.0,
    least: 1.0,
};

/// Each side in the order `list` names them, with its words.
const SIDES: [(Side, &str); 4] = [
    (Side::Left, "on its left"),
    (Side::Right, "on its right"),
    (Side::Up, "above it"),
    (Side::Down, "below it"),
];

/// Whether every child of every branch of `tree` keeps the least share of its branch.
fn roomy(tree: &Tree) -> bool {
    match tree {
        Tree::Leaf { .. } | Tree::Stack { .. } => true,
        Tree::Branch { children, .. } => {
            let whole: f64 = children.iter().map(|child| child.share).sum();
            (children.iter()).all(|child| child.share / whole >= LEAST - 1e-9 && roomy(&child.tree))
        }
    }
}

/// `names` as words: `t1`, `t1 and t2`, `t1, t2 and t3`.
fn listed(names: &[String]) -> String {
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

impl Desk {
    /// Where `open` puts a tab of `kind` that `spec` asks for `beside` a pane: none without
    /// `beside`. Refused, with what to do instead, for a pane not of the caller's project or in
    /// no tab, a side but right or down, a document asked under a pane (it joins the documents
    /// of the tab, on the right of its terminals) or beside a tab that holds none, a kind with a
    /// tab of its own (the board), and a terminal with no room left beside the pane.
    pub(in crate::desk) fn beside_of(
        &self,
        call: &Call,
        spec: &Value,
        kind: Kind,
    ) -> Result<Option<Placed>, String> {
        let Some(key) = text(spec, "beside") else {
            return match text(spec, "side") {
                Some(_) => Err("`side` goes with `beside`: the tab to open it beside".into()),
                None => Ok(None),
            };
        };
        let side = match text(spec, "side") {
            None | Some("right") => Side::Right,
            Some("down") => Side::Down,
            Some(other) => return Err(format!("`side` is right or down, not `{other}`")),
        };
        if matches!(kind, Kind::Board | Kind::Made(_)) {
            return Err(format!(
                "`beside` is for a terminal, a page or a file: a {} has a tab of its own",
                kind.name()
            ));
        }
        let found = crate::lock(&self.ledger).get(key).copied();
        let Some((row, _)) = found.filter(|(_, project)| *project == call.project.id) else {
            return Err(format!(
                "{key} is no tab of this project: `list` names yours"
            ));
        };
        let read = (self.feed.store()).read(|db| {
            let Some(id) = layout::tab_of(db, row)? else {
                return Ok(None);
            };
            let Some(tab) = layout::tab(db, id)? else {
                return Ok(None);
            };
            let takes = crate::desk::beside::takes(db, &tab.tree, row)?;
            Ok(Some((tab.tree, takes)))
        })?;
        let Some((tree, takes)) = read else {
            return Err(format!("{key} is in no tab of the window"));
        };
        if matches!(kind, Kind::Web | Kind::File) {
            if side == Side::Down {
                return Err(
                    "a document joins the documents of the tab beside, on the right of \
                            its terminals: `side` down is for a terminal"
                        .into(),
                );
            }
            if !takes {
                return Err(format!(
                    "{key}'s tab holds no documents: open a page or a file beside a terminal"
                ));
            }
        } else {
            let mut split = tree;
            let room = split.split(&row, Row::MIN, side.across(), side.forward()) && roomy(&split);
            if !room {
                let way = if side == Side::Down {
                    "below it"
                } else {
                    "on its right"
                };
                return Err(format!(
                    "{key} has no room for another pane {way}: open it in a tab of its own, \
                     without `beside`"
                ));
            }
        }
        Ok(Some(Placed {
            key: key.to_string(),
            row,
            side,
        }))
    }

    /// Where pane `key` is, as `list` says it: `in tab 2` (its place among its window's tabs
    /// of the project, as ⌘1…9 count them), the pane its tab is zoomed on, and the panes on
    /// each side of it (`t1 on its left`), else the document a stack shows over it. None for a
    /// pane in no tab.
    pub(in crate::desk) fn placed_words(&self, key: &str) -> Option<String> {
        let keys: std::collections::HashMap<Row, String> = (crate::lock(&self.ledger).iter())
            .map(|(key, (row, _))| (*row, key.clone()))
            .collect();
        let row = keys
            .iter()
            .find(|(_, it)| *it == key)
            .map(|(row, _)| *row)?;
        let read = (self.feed.store()).read(|db| {
            let Some(id) = layout::tab_of(db, row)? else {
                return Ok(None);
            };
            let Some(tab) = layout::tab(db, id)? else {
                return Ok(None);
            };
            let tabs = layout::tabs(db, tab.workspace)?;
            let place = tabs
                .iter()
                .position(|it| it.id == id)
                .map_or(1, |at| at + 1);
            Ok(Some((tab, place)))
        });
        let (tab, place) = read.ok().flatten()?;
        let name = |row: &Row| keys.get(row).cloned();
        let mut said = format!("in tab {place}");
        if tab.whole
            && tab.tree.visible().len() >= 2
            && let Some(on) = tab.focused.as_ref().and_then(name)
        {
            let _ = write!(said, ", zoomed on {on}");
        }
        let sight = tab.tree.visible();
        if !sight.contains(&row) {
            let over = (tab.tree.stacked().contains(&row))
                .then(|| sight.iter().find(|pane| tab.tree.stacked().contains(pane)))
                .flatten()
                .and_then(name);
            if let Some(over) = over {
                let _ = write!(said, ", behind {over}");
            }
            return Some(said);
        }
        let laid = geometry::lay(&tab.tree, Rect::new(0.0, 0.0, 1000.0, 1000.0), LISTED);
        for (side, words) in SIDES {
            let names: Vec<String> = (geometry::beside(&laid.panes, &row, side, LISTED.gap).iter())
                .filter_map(name)
                .collect();
            if !names.is_empty() {
                let _ = write!(said, ", {} {words}", listed(&names));
            }
        }
        Some(said)
    }
}

#[cfg(test)]
#[path = "layout_agent_tests.rs"]
mod tests;
