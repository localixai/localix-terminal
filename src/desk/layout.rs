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
fn told(write: &Write<'_, '_>, entities: &[Entity]) -> Result<(), String> {
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
    /// not opened again across a restart yet.)
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
    fn workspace_for(
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
    /// so the tab is the two of them.
    pub(super) fn split_panes(&self, shown: Row, beside: Row, right: bool) -> Result<(), String> {
        let now = crate::board::now();
        self.feed.write(|write| {
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

    /// The tab of pane `pane` is the one its workspace shows, `pane` in focus.
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
            if tab.focused != Some(pane) {
                layout::set_tree(write.tx(), id, &tab.tree, Some(pane))?;
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

    /// The person split the window in project `group` (`halves`, by its panes' keys), or made it
    /// whole there: the layout's tab of the pane in use made of the two, or each split tab of
    /// the project made whole, the pane in focus staying.
    pub(super) fn split_told(&self, group: &str, halves: Option<crate::browser::Halves<'_>>) {
        // The app ending, its panes go: the layout stays as it is for when they open again.
        if self.ending.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let row = |key: &str| crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        let done = match halves {
            Some(halves) => match (row(halves.shown), row(halves.beside)) {
                (Some(shown), Some(beside)) => self.split_panes(shown, beside, halves.right),
                _ => Ok(()),
            },
            None => match crate::projects::of_key(group) {
                Some(project) => self.unsplit_project(project),
                None => Ok(()),
            },
        };
        if let Err(why) = done {
            log::warn!("the split of {group} is not in the layout: {why}");
        }
    }

    /// Every split tab of project `project` in the window in use made whole, its pane in focus
    /// staying.
    fn unsplit_project(&self, project: ProjectId) -> Result<(), String> {
        let now = crate::board::now();
        let wanted = self.in_use();
        self.feed.write(|write| {
            let open = layout::windows(write.tx())?;
            let window = (wanted.filter(|wanted| open.iter().any(|window| window.id == *wanted)))
                .or_else(|| open.first().map(|window| window.id));
            let Some(window) = window else {
                return Ok(());
            };
            let space = (layout::workspaces(write.tx(), window)?.into_iter())
                .find(|space| space.project == project);
            let Some(space) = space else {
                return Ok(());
            };
            for tab in layout::tabs(write.tx(), space.id)? {
                let panes = tab.tree.panes();
                if panes.len() > 1 {
                    let kept = tab.focused.unwrap_or(panes[0]);
                    Self::unsplit_tab(write, tab.id, kept, now)?;
                }
            }
            Ok(())
        })
    }
}

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

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
