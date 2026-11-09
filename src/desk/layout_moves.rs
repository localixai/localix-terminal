//! Move a tab between places or windows, committing its layout change and notices together.

use super::Desk;
use super::layout::told;
use crate::feed::Entity;
use crate::layout::{self, WindowId};
use crate::ledger::Row;

impl Desk {
    /// The person moved the tab of pane `key` before the tab of pane `before` (to the end with
    /// none): in its own list, or let go over the list of `window`, which it moves into (its
    /// project's workspace there, shown). A window the move left with no tab is not closed here:
    /// the window that closed tells it.
    ///
    /// In one go, for tests: the window's events find the rows at once and keep the move on the
    /// writer thread (heard.rs).
    #[cfg(test)]
    pub(super) fn moved(&self, key: &str, before: Option<&str>, window: Option<WindowId>) {
        if let Some(rows) = self.rows_moved(key, before) {
            self.move_kept(key, rows, window);
        }
    }

    /// The rows of pane `key` and of pane `before`, as the ledger has them now: none when `key`
    /// has none.
    pub(super) fn rows_moved(&self, key: &str, before: Option<&str>) -> Option<(Row, Option<Row>)> {
        let row = |key: &str| crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        Some((row(key)?, before.and_then(row)))
    }

    /// The move of pane `pane` (`key`) before pane `before` kept in the layout, as `moved` says.
    pub(super) fn move_kept(
        &self,
        key: &str,
        (pane, before): (Row, Option<Row>),
        window: Option<WindowId>,
    ) {
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
    /// In one go, for tests, as `moved` is.
    #[cfg(test)]
    pub(super) fn detached(&self, key: &str) {
        if let Some((pane, _)) = self.rows_moved(key, None) {
            self.detach_kept(key, pane);
        }
    }

    /// Pane `pane` (`key`) a window of its own in the layout, as `detached` says.
    pub(super) fn detach_kept(&self, key: &str, pane: Row) {
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
