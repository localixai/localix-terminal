//! The layout's owner (layout.rs, docs/windows.md): the windows, their workspaces, the tabs of
//! each and the panes of each tab. Every change here is one transaction with its notices, written
//! by the desk alone: a pane placed in a tab as it opens, taken out as it closes, put beside
//! another (a split), a tab moved or shown. The window draws what this keeps.

use crate::feed::{Entity, Op, Write};
use crate::layout::{self, Tree, WindowId, WorkspaceId};
use crate::ledger::Row;
use crate::projects::ProjectId;

use super::{Call, Desk};

/// The notices of a change: each thing it changed, upserted.
pub(super) fn told(write: &Write<'_, '_>, entities: &[Entity]) -> Result<(), String> {
    for entity in entities {
        write.record(entity, Op::Upserted)?;
    }
    Ok(())
}

impl Desk {
    /// A new window, on `call`'s project, with the person's shell in its folder: the layout's
    /// first, then the window's (the app told to make it, it is the one in use), then the shell
    /// in it.
    pub(super) fn new_window(&self, call: &Call<'_>, group: &str) -> Result<(), String> {
        let now = crate::board::now();
        let window = self.feed.write(|write| {
            let window = layout::open_window(write.tx(), now)?;
            let space = layout::workspace_of(write.tx(), window, call.project.id, now)?;
            layout::show_workspace(write.tx(), window, space)?;
            told(write, &[Entity::Window { id: window }])?;
            Ok(window)
        })?;
        if let Some(browser) = &self.browser {
            browser.new_window(window);
        }
        let call = Call {
            window: Some(window),
            ..*call
        };
        self.person_terminal(&call, group, "")
    }

    /// The app starts with one window: the tabs other windows had when it quit are gathered into
    /// the first, each in its project's workspace there, and those windows close. (Windows are
    /// not opened again across a restart yet.) Gathered, its tabs are shaped as sessions keep
    /// them (sessions.rs).
    pub(super) fn gather_windows(&self) {
        let now = crate::board::now();
        let gathered = self.feed.write(|write| {
            let open = layout::windows(write.tx())?;
            let Some((first, rest)) = open.split_first() else {
                return Ok(());
            };
            for window in rest {
                let spaces = layout::gather(write.tx(), window.id, first.id, now)?;
                write.record(&Entity::Window { id: window.id }, Op::Deleted)?;
                let spaces: Vec<Entity> = (spaces.into_iter())
                    .map(|id| Entity::Workspace { id })
                    .collect();
                told(write, &spaces)?;
            }
            Ok(())
        });
        if let Err(why) = gathered {
            log::warn!("the windows of the run before stay as they were: {why}");
        }
        self.sessions_shaped();
    }

    /// The person closed window `window` (its tabs closed first): the layout closes it.
    pub(super) fn window_closed(&self, window: WindowId) {
        if window == 0 {
            return;
        }
        let now = crate::board::now();
        let closed = self.feed.write(|write| {
            layout::close_window(write.tx(), window, now)?;
            write.record(&Entity::Window { id: window }, Op::Deleted)
        });
        if let Err(why) = closed {
            log::warn!("window {window} stays open in the layout: {why}");
        }
    }

    /// The window the person works in, as the window tells it.
    pub(super) fn in_use(&self) -> Option<WindowId> {
        self.browser
            .as_ref()
            .and_then(crate::browser::Browser::in_use)
    }

    /// The window panes open in: `wanted` while it is open, else the first open, else one opened
    /// `now`.
    pub(super) fn the_window(
        write: &Write<'_, '_>,
        now: u64,
        wanted: Option<WindowId>,
    ) -> Result<WindowId, String> {
        let open = layout::windows(write.tx())?;
        if let Some(window) = wanted.filter(|wanted| open.iter().any(|window| window.id == *wanted))
        {
            return Ok(window);
        }
        if let Some(window) = open.first() {
            return Ok(window.id);
        }
        let window = layout::open_window(write.tx(), now)?;
        told(write, &[Entity::Window { id: window }])?;
        Ok(window)
    }

    /// The workspace of `project` in the window panes open in, made when it has none.
    pub(super) fn workspace_for(
        write: &Write<'_, '_>,
        project: ProjectId,
        now: u64,
        window: Option<WindowId>,
    ) -> Result<WorkspaceId, String> {
        let window = Self::the_window(write, now, window)?;
        let made = layout::workspace_of(write.tx(), window, project, now)?;
        let shown = layout::window(write.tx(), window)?.and_then(|window| window.shown);
        if shown.is_none() {
            layout::show_workspace(write.tx(), window, made)?;
            told(write, &[Entity::Window { id: window }])?;
        }
        Ok(made)
    }

    /// Pane `pane` of `project`, opened in `window` (none: the first): a tab of its own, the last
    /// of its workspace's, unless it is in a tab already (one kept across a restart).
    pub(super) fn place_pane(
        write: &Write<'_, '_>,
        pane: Row,
        (project, window): (ProjectId, Option<WindowId>),
        now: u64,
    ) -> Result<(), String> {
        if layout::tab_of(write.tx(), pane)?.is_some() {
            return Ok(());
        }
        let workspace = Self::workspace_for(write, project, now, window)?;
        layout::open_tab(write.tx(), workspace, &Tree::leaf(pane), None, None, now)?;
        told(write, &[Entity::Workspace { id: workspace }])
    }

    /// Pane `pane` closed: out of its tab, which closes when it was all of it.
    pub(super) fn unplace_pane(write: &Write<'_, '_>, pane: Row, now: u64) -> Result<(), String> {
        let Some(id) = layout::tab_of(write.tx(), pane)? else {
            return Ok(());
        };
        let Some(tab) = layout::tab(write.tx(), id)? else {
            return Ok(());
        };
        match tab.tree.without(pane) {
            Some(tree) => {
                let focused = tab.focused.filter(|focused| *focused != pane);
                layout::set_tree(write.tx(), id, &tree, focused)?;
                told(write, &[Entity::Tab { id }])
            }
            None => {
                layout::close_tab(write.tx(), id, now)?;
                told(write, &[Entity::Workspace { id: tab.workspace }])
            }
        }
    }

    /// Pane `beside` put beside pane `shown`, on its right or its left, in `shown`'s tab: out
    /// of the tab it was in (which closes when it was all of it). Each pane alone again first,
    /// so the tab is the two of them. A session and a document are no such pair: the document
    /// is shown in the session, or joins it (layout_split.rs).
    pub(super) fn split_panes(&self, shown: Row, beside: Row, right: bool) -> Result<(), String> {
        let now = crate::board::now();
        self.feed.write(|write| {
            if Self::split_session(write, (shown, beside), right, now)? {
                return Ok(());
            }
            let Some(id) = layout::tab_of(write.tx(), shown)? else {
                return Err("the pane in use is in no tab".to_string());
            };
            if layout::tab_of(write.tx(), beside)? == Some(id) {
                Self::unsplit_tab(write, id, shown, now)?;
            }
            Self::unplace_pane(write, beside, now)?;
            let Some(tab) = layout::tab(write.tx(), id)? else {
                return Err("the pane in use is in no tab".to_string());
            };
            let mut tree = tab.tree;
            if !tree.split(shown, beside, true, right) {
                return Err("the pane in use is not in its tab".to_string());
            }
            layout::set_tree(write.tx(), id, &tree, Some(shown))?;
            told(
                write,
                &[Entity::Tab { id }, Entity::Workspace { id: tab.workspace }],
            )
        })
    }

    /// The tab of pane `kept` made of it alone: each other pane a tab of its own after it, in
    /// the order they were in.
    pub(super) fn unsplit_tab(
        write: &Write<'_, '_>,
        id: layout::TabId,
        kept: Row,
        now: u64,
    ) -> Result<(), String> {
        let Some(tab) = layout::tab(write.tx(), id)? else {
            return Ok(());
        };
        let others: Vec<Row> = (tab.tree.panes().into_iter())
            .filter(|pane| *pane != kept)
            .collect();
        if others.is_empty() {
            return Ok(());
        }
        layout::set_tree(write.tx(), id, &Tree::leaf(kept), Some(kept))?;
        for (after, pane) in others.into_iter().enumerate() {
            let at = Some(tab.place + 1 + after);
            layout::open_tab(write.tx(), tab.workspace, &Tree::leaf(pane), None, at, now)?;
        }
        told(
            write,
            &[Entity::Tab { id }, Entity::Workspace { id: tab.workspace }],
        )
    }

    /// The window panes open in shows its workspace of project `project` (made when it has
    /// none).
    pub(super) fn show_project(
        write: &Write<'_, '_>,
        (project, window): (ProjectId, Option<WindowId>),
        now: u64,
    ) -> Result<(), String> {
        let window = Self::the_window(write, now, window)?;
        let space = layout::workspace_of(write.tx(), window, project, now)?;
        let shown = layout::window(write.tx(), window)?.and_then(|window| window.shown);
        if shown != Some(space) {
            layout::show_workspace(write.tx(), window, space)?;
            told(write, &[Entity::Window { id: window }])?;
        }
        Ok(())
    }

    /// The tab of pane `pane` is the one its workspace shows, `pane` in focus: a document, the
    /// one its session has in sight.
    pub(super) fn show_pane(&self, pane: Row) -> Result<(), String> {
        self.feed.write(|write| {
            let Some(id) = layout::tab_of(write.tx(), pane)? else {
                return Ok(());
            };
            let Some(tab) = layout::tab(write.tx(), id)? else {
                return Ok(());
            };
            let shown = layout::workspace(write.tx(), tab.workspace)?.and_then(|space| space.shown);
            let mut changed = vec![];
            if shown != Some(id) {
                layout::show_tab(write.tx(), tab.workspace, Some(id))?;
                changed.push(Entity::Workspace { id: tab.workspace });
            }
            let mut tree = tab.tree.clone();
            tree.show(pane);
            if tab.focused != Some(pane) || tree != tab.tree {
                layout::set_tree(write.tx(), id, &tree, Some(pane))?;
                changed.push(Entity::Tab { id });
            }
            told(write, &changed)
        })
    }
}

impl Desk {
    /// Project `project`'s panes in every window, in the order the person sees them: window by
    /// window, by their tabs' places, and in each tab left to right.
    pub(super) fn panes_in_order(
        db: &crate::store::Connection,
        project: ProjectId,
    ) -> Result<Vec<Row>, String> {
        let mut panes = vec![];
        for window in layout::windows(db)? {
            let spaces = layout::workspaces(db, window.id)?.into_iter();
            for space in spaces.filter(|space| space.project == project) {
                let tabs = layout::tabs(db, space.id)?.into_iter();
                panes.extend(tabs.flat_map(|tab| tab.tree.panes()));
            }
        }
        Ok(panes)
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
