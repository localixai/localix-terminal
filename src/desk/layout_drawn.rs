//! The layout projected into browser tab keys, with one feed consumer keeping it current.

use super::Desk;
use crate::feed::Entity;
use crate::layout::{self, Tree, WindowId};
use crate::ledger::Row;

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
                    group: crate::projects::key(space.project),
                    tabs,
                    shown,
                }
            })
            .collect();
        browser.arranged(windows, arranged);
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

/// The feed's consumer that keeps the window drawing the layout: each change of a window, a
/// workspace, a tab or a pane's key, and the window is told the layout again (it pulls; the
/// notice carries nothing of it).
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
        }
    }
}
