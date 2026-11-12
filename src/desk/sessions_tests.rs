//! Tests of the tabs a run before left, brought to the shape sessions keep: a document beside
//! a terminal is that terminal's, documents alone join a session of their project, and where it
//! has none they are one tab.

use std::sync::Arc;

use super::super::Desk;
use super::super::tests::desk_on;
use crate::layout::{self, Share, Tree};
use crate::ledger::{self, Kind, Opened};
use crate::testing::Scratch;

/// A pane of `kind` in project `project`, open in the ledger.
fn pane(desk: &Desk, project: crate::projects::ProjectId, kind: Kind) -> i64 {
    let opened = Opened {
        project,
        kind,
        agent: None,
        title: "",
        url: None,
        cwd: None,
        command: None,
    };
    (desk.feed.store())
        .write(|tx| ledger::open(tx, &opened, 1))
        .unwrap()
}

#[test]
fn the_tabs_of_a_run_before_are_shaped_as_sessions_keep_them() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(Arc::clone(&store)), Scratch::new());
    let project = desk.project(scratch.path()).unwrap().id;
    let [term, page, file, other, board] = [
        Kind::Terminal,
        Kind::Page,
        Kind::File,
        Kind::Page,
        Kind::Board,
    ]
    .map(|kind| pane(&desk, project, kind));
    let half = |pane: i64| Share {
        share: 0.5,
        tree: Tree::leaf(pane),
    };
    let space = store
        .write(|tx| {
            let window = layout::open_window(tx, 1)?;
            let space = layout::workspace_of(tx, window, project, 1)?;
            // A terminal with a page on its right, as the run before kept what an agent opened.
            let split = Tree::Branch {
                across: true,
                children: vec![half(term), half(page)],
            };
            layout::open_tab(tx, space, &split, Some(term), None, 1)?;
            layout::open_tab(tx, space, &Tree::leaf(file), None, None, 1)?;
            layout::open_tab(tx, space, &Tree::leaf(board), None, None, 1)?;
            let shown = layout::open_tab(tx, space, &Tree::leaf(other), None, None, 1)?;
            layout::show_tab(tx, space, Some(shown))?;
            Ok(space)
        })
        .unwrap();

    desk.sessions_shaped();
    let read = || store.read(|db| layout::tabs(db, space)).unwrap();
    let tabs = read();
    let seen: Vec<Vec<i64>> = tabs.iter().map(|tab| tab.tree.visible()).collect();
    // The page is the terminal's document, where it was, and the ones that stood alone join
    // it: the one that was shown is the one in sight, in the session the project shows now.
    // The board is as it was.
    assert_eq!(seen, [vec![term, other], vec![board]]);
    assert_eq!(tabs[0].tree.stacked(), [page, file, other]);
    assert_eq!(tabs[0].focused, Some(term));
    let shown = store
        .read(|db| Ok(layout::workspace(db, space)?.and_then(|space| space.shown)))
        .unwrap();
    assert_eq!(shown, Some(tabs[0].id));

    // Shaped once: a second pass changes nothing.
    desk.sessions_shaped();
    assert_eq!(read(), tabs);
    desk.shutdown();
}

#[test]
fn documents_alone_are_one_tab_while_their_project_has_no_session() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let (desk, scratch) = (desk_on(Arc::clone(&store)), Scratch::new());
    let project = desk.project(scratch.path()).unwrap().id;
    let [page, file, board] =
        [Kind::Page, Kind::File, Kind::Board].map(|kind| pane(&desk, project, kind));
    let space = store
        .write(|tx| {
            let window = layout::open_window(tx, 1)?;
            let space = layout::workspace_of(tx, window, project, 1)?;
            for pane in [page, board, file] {
                layout::open_tab(tx, space, &Tree::leaf(pane), None, None, 1)?;
            }
            Ok(space)
        })
        .unwrap();

    desk.sessions_shaped();
    let tabs = store.read(|db| layout::tabs(db, space)).unwrap();
    let seen: Vec<Vec<i64>> = tabs.iter().map(|tab| tab.tree.visible()).collect();
    assert_eq!(seen, [vec![page], vec![board]]);
    assert_eq!(tabs[0].tree.stacked(), [page, file]);
    desk.shutdown();
}
