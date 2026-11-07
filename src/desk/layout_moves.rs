//! Move a tab between places or windows, committing its layout change and notices together.

use super::Desk;
use super::layout::told;
use crate::feed::Entity;
use crate::layout::{self, WindowId};

impl Desk {
    /// The person moved the tab of pane `key` before the tab of pane `before` (to the end with
    /// none): in its own list, or let go over the list of `window`, which it moves into (its
    /// project's workspace there, shown). A window the move left with no tab is not closed here:
    /// the window that closed tells it.
    pub(super) fn moved(&self, key: &str, before: Option<&str>, window: Option<WindowId>) {
        let row = |key: &str| crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        let Some(pane) = row(key) else {
            return;
        };
        let before = before.and_then(row);
        let now = crate::board::now();
        let moved = self.feed.write(|write| {
            let Some(id) = layout::tab_of(write.tx(), pane)? else {
                return Ok(());
            };
            let Some(tab) = layout::tab(write.tx(), id)? else {
                return Ok(());
            };
            let into = match window {
                Some(window) => {
                    let Some(from) = layout::workspace(write.tx(), tab.workspace)? else {
                        return Ok(());
                    };
                    layout::workspace_of(write.tx(), window, from.project, now)?
                }
                None => tab.workspace,
            };
            let tabs = layout::tabs(write.tx(), into)?;
            let others: Vec<layout::TabId> = (tabs.iter())
                .map(|other| other.id)
                .filter(|other| *other != id)
                .collect();
            let target = before
                .map(|before| layout::tab_of(write.tx(), before))
                .transpose()?
                .flatten();
            let at = (target.and_then(|target| others.iter().position(|other| *other == target)))
                .unwrap_or(others.len());
            if into == tab.workspace && tabs.iter().position(|other| other.id == id) == Some(at) {
                return Ok(());
            }
            layout::move_tab(write.tx(), id, into, at)?;
            if into == tab.workspace {
                return told(write, &[Entity::Workspace { id: into }]);
            }
            if let Some(window) = window {
                layout::show_tab(write.tx(), into, Some(id))?;
                layout::show_workspace(write.tx(), window, into)?;
            }
            told(
                write,
                &[
                    Entity::Workspace { id: tab.workspace },
                    Entity::Workspace { id: into },
                ],
            )
        });
        if let Err(why) = moved {
            log::warn!("tab {key} did not move in the layout: {why}");
        }
    }

    /// The person let the tab of pane `key` go away from every list: it is a window of its own
    /// now, showing the tab's project with that tab only. The window it is in tells its row.
    pub(super) fn detached(&self, key: &str) {
        let Some(pane) = crate::lock(&self.ledger).get(key).map(|(row, _)| *row) else {
            return;
        };
        let now = crate::board::now();
        let window = self.feed.write(|write| {
            let Some(id) = layout::tab_of(write.tx(), pane)? else {
                return Ok(None);
            };
            let Some(tab) = layout::tab(write.tx(), id)? else {
                return Ok(None);
            };
            let Some(from) = layout::workspace(write.tx(), tab.workspace)? else {
                return Ok(None);
            };
            let window = layout::open_window(write.tx(), now)?;
            let into = layout::workspace_of(write.tx(), window, from.project, now)?;
            layout::move_tab(write.tx(), id, into, 0)?;
            layout::show_tab(write.tx(), into, Some(id))?;
            layout::show_workspace(write.tx(), window, into)?;
            told(
                write,
                &[
                    Entity::Window { id: window },
                    Entity::Workspace { id: from.id },
                    Entity::Workspace { id: into },
                ],
            )?;
            Ok(Some(window))
        });
        match window {
            Ok(Some(window)) => {
                if let Some(browser) = &self.browser {
                    browser.new_window(window);
                }
            }
            Ok(None) => {}
            Err(why) => log::warn!("tab {key} did not become a window: {why}"),
        }
    }
}
