//! The layout projected into browser tab keys, and the one feed consumer that keeps the window
//! current: its layout, each project's past and the projects it lists.

use super::Desk;
use crate::browser::{Ledger, Past, TabKind, Visited};
use crate::feed::Entity;
use crate::layout::{self, Tree, WindowId};
use crate::ledger::{self, Kind, Row};
use crate::projects::{self, ProjectId};

/// How many of a project's panes the history menu lists, and of its pages the address field
/// suggests.
const LISTED: usize = 50;

impl Desk {
    /// The window told its projects' tabs as the layout keeps them, by their panes' keys: a pane
    /// with no key this run (not open yet) is left out.
    pub(super) fn tell_arranged(&self) {
        let Some(browser) = self.browser.as_ref() else {
            return;
        };
        let keys: std::collections::HashMap<Row, String> = (crate::lock(&self.ledger).iter())
            .map(|(key, (row, _))| (*row, key.clone()))
            .collect();
        let read = self.feed.store().read(|db| {
            let windows: Vec<WindowId> = (layout::windows(db)?.iter())
                .map(|window| window.id)
                .collect();
            let mut all = vec![];
            for window in &windows {
                for space in layout::workspaces(db, *window)? {
                    let tabs = layout::tabs(db, space.id)?;
                    all.push((*window, space, tabs));
                }
            }
            Ok((windows, all))
        });
        let (windows, spaces) = match read {
            Ok(read) => read,
            Err(why) => return log::warn!("the layout could not be read: {why}"),
        };
        let arranged = (spaces.into_iter())
            .map(|(window, space, tabs)| {
                let shown = (tabs.iter())
                    .find(|tab| Some(tab.id) == space.shown)
                    .and_then(|tab| tab.focused)
                    .and_then(|pane| keys.get(&pane).cloned());
                let tabs = (tabs.iter())
                    .filter_map(|tab| arranging(tab, &keys))
                    .collect();
                crate::browser::Arranged {
                    window,
                    group: projects::key(space.project),
                    tabs,
                    shown,
                }
            })
            .collect();
        browser.arranged(windows, arranged);
    }

    /// The window told project `project`'s past: its panes, the latest first, each open one with
    /// its key; and its pages. Only the window's consumer of the feed calls it.
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
        // The key of each open pane listed, found in one pass over the panes open now.
        let listed: std::collections::HashSet<Row> = (tabs.iter())
            .filter(|tab| tab.closed.is_none())
            .map(|tab| tab.row)
            .collect();
        let open: std::collections::HashMap<Row, String> = (crate::lock(&self.ledger).iter())
            .filter(|(_, (row, _))| listed.contains(row))
            .map(|(key, (row, _))| (*row, key.clone()))
            .collect();
        let key_of = |row: Row| open.get(&row).cloned();
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

/// Tab `tab` by its panes' keys; none when none of its panes has one.
fn arranging(
    tab: &layout::Tab,
    keys: &std::collections::HashMap<Row, String>,
) -> Option<crate::browser::Arranging> {
    let (across, shares) = match &tab.tree {
        Tree::Leaf { .. } => (true, vec![1.0]),
        Tree::Branch { across, children } => {
            (*across, children.iter().map(|child| child.share).collect())
        }
    };
    let panes: Vec<String> = (tab.tree.panes().into_iter())
        .filter_map(|pane| keys.get(&pane).cloned())
        .collect();
    let focused = tab.focused.and_then(|pane| keys.get(&pane).cloned());
    (!panes.is_empty()).then_some(crate::browser::Arranging {
        panes,
        across,
        shares,
        focused,
    })
}

/// The feed's consumer that keeps the window current, and the one part that tells it what follows:
/// each change of a window, a workspace, a tab or a pane's key, and it is told the layout again;
/// of a project's ledger, that project's past (its history menu, its address field); of the
/// projects, the switcher's list. It pulls; the notice carries nothing of it.
pub(super) struct Drawn {
    pub(super) desk: std::sync::Weak<Desk>,
}

impl crate::feed::Consumer for Drawn {
    fn name(&self) -> &'static str {
        "window"
    }

    fn kind(&self) -> crate::feed::Kind {
        crate::feed::Kind::Reactor
    }

    fn handle(&mut self, notice: &crate::feed::Notice) -> Result<(), String> {
        let Some(desk) = self.desk.upgrade() else {
            return Ok(());
        };
        match &notice.entity {
            Entity::Window { .. }
            | Entity::Workspace { .. }
            | Entity::Tab { .. }
            | Entity::Pane { .. } => desk.tell_arranged(),
            Entity::Ledger { project } => desk.tell_ledger(*project),
            Entity::Projects => desk.tell_places(),
            Entity::Said { .. }
            | Entity::Board { .. }
            | Entity::Terminal { .. }
            | Entity::Ask { .. } => {}
        }
        Ok(())
    }

    fn reconcile(&mut self) {
        if let Some(desk) = self.desk.upgrade() {
            desk.tell_arranged();
            desk.tell_places();
            for project in desk.known_projects() {
                desk.tell_ledger(project);
            }
        }
    }
}

#[cfg(test)]
#[path = "layout_drawn_tests.rs"]
mod tests;
