//! Tests of the layout's owner: a pane opened is a tab, a split is one tab of two panes kept
//! across a restart, and each change is told in the feed with it.

use std::sync::Arc;

use serde_json::json;

use super::super::tests::{caller, desk_on};
use crate::browser::Halves;
use crate::layout;
use crate::testing::Scratch;

const TERMINAL: &str =
    r#"{"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200}"#;

/// The open tabs of project `project`'s workspace, each as its panes.
fn tabs(store: &crate::store::Store, project: crate::projects::ProjectId) -> Vec<Vec<i64>> {
    store
        .read(|db| {
            let window = layout::windows(db)?.first().map(|window| window.id);
            let Some(window) = window else {
                return Ok(vec![]);
            };
            let space = (layout::workspaces(db, window)?.into_iter())
                .find(|space| space.project == project);
            let Some(space) = space else {
                return Ok(vec![]);
            };
            Ok((layout::tabs(db, space.id)?.into_iter())
                .map(|tab| tab.tree.panes())
                .collect())
        })
        .unwrap()
}

/// The panes of project `project`'s tabs in window `window`, in their places.
fn tabs_in(
    store: &crate::store::Store,
    window: layout::WindowId,
    project: crate::projects::ProjectId,
) -> Vec<Vec<i64>> {
    store
        .read(|db| {
            let space = (layout::workspaces(db, window)?.into_iter())
                .find(|space| space.project == project);
            let Some(space) = space else {
                return Ok(vec![]);
            };
            Ok((layout::tabs(db, space.id)?.into_iter())
                .map(|tab| tab.tree.panes())
                .collect())
        })
        .unwrap()
}

/// The kinds of the notices in the feed, oldest first.
fn notices(store: &crate::store::Store) -> Vec<String> {
    store
        .read(|db| {
            let mut query = db
                .prepare("SELECT json_extract(entity, '$.kind') FROM change_events ORDER BY seq")
                .map_err(|why| why.to_string())?;
            let kinds = query
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|why| why.to_string())?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|why| why.to_string())?;
            Ok(kinds)
        })
        .unwrap()
}

#[test]
fn panes_opened_are_tabs_and_a_split_is_one_tab_of_two_kept_across_a_restart() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open: serde_json::Value = serde_json::from_str(TERMINAL).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    let project = mine.project.id;
    let row = |key: &str| {
        crate::lock(&desk.ledger)
            .get(key)
            .map(|(row, _)| *row)
            .unwrap()
    };
    let (one, two, three) = (row("t1"), row("t2"), row("t3"));
    assert_eq!(tabs(&store, project), [vec![one], vec![two], vec![three]]);
    let kinds = notices(&store);
    for kind in ["window", "workspace"] {
        assert!(kinds.iter().any(|told| told == kind), "{kinds:?}");
    }

    // The person puts t3 on the right of t1: one tab of the two, where t1 was.
    let group = crate::projects::key(project);
    let halves = Halves {
        shown: "t1",
        beside: "t3",
        right: true,
    };
    desk.split_told(halves);
    assert_eq!(tabs(&store, project), [vec![one, three], vec![two]]);
    assert!(notices(&store).iter().any(|told| told == "tab"));

    // The app quits and comes back: the same panes in the same tabs, split as they were.
    desk.shutdown();
    let again = desk_on(Arc::clone(&store));
    let mine = caller(&again, scratch.path(), None);
    again.reopen_project(&mine.call(), &group).unwrap();
    assert_eq!(tabs(&store, project), [vec![one, three], vec![two]]);

    // One half closed: the tab is the other alone.
    let key_of = |row: i64| {
        (crate::lock(&again.ledger).iter())
            .find(|(_, (kept, _))| *kept == row)
            .map(|(key, _)| key.clone())
            .unwrap()
    };
    let closing = json!({"action": "close", "tab": key_of(three)});
    again.window(&mine.call(), &closing).unwrap();
    assert_eq!(tabs(&store, project), [vec![one], vec![two]]);
    again.shutdown();
}

#[test]
fn a_split_made_whole_puts_the_other_pane_in_a_tab_after_it() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open: serde_json::Value = serde_json::from_str(TERMINAL).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    let project = mine.project.id;
    let row = |key: &str| {
        crate::lock(&desk.ledger)
            .get(key)
            .map(|(row, _)| *row)
            .unwrap()
    };
    let (one, two) = (row("t1"), row("t2"));
    desk.split_told(Halves {
        shown: "t2",
        beside: "t1",
        right: false,
    });
    assert_eq!(tabs(&store, project), [vec![one, two]]);
    desk.whole_told(Halves {
        shown: "t2",
        beside: "t1",
        right: false,
    });
    assert_eq!(tabs(&store, project), [vec![two], vec![one]]);
    desk.shutdown();
}

#[test]
fn a_tab_dragged_before_another_moves_there_and_to_the_end_with_none() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open: serde_json::Value = serde_json::from_str(TERMINAL).unwrap();
    for _ in 0..3 {
        desk.window(&mine.call(), &open).unwrap();
    }
    let project = mine.project.id;
    let row = |key: &str| {
        crate::lock(&desk.ledger)
            .get(key)
            .map(|(row, _)| *row)
            .unwrap()
    };
    let (one, two, three) = (row("t1"), row("t2"), row("t3"));
    assert_eq!(tabs(&store, project), [vec![one], vec![two], vec![three]]);
    desk.moved("t3", Some("t1"), None);
    assert_eq!(tabs(&store, project), [vec![three], vec![one], vec![two]]);
    desk.moved("t3", None, None);
    assert_eq!(tabs(&store, project), [vec![one], vec![two], vec![three]]);
    // Before itself, nothing moves and nothing is told.
    let told = notices(&store).len();
    desk.moved("t2", Some("t3"), None);
    assert_eq!(tabs(&store, project), [vec![one], vec![two], vec![three]]);
    assert_eq!(notices(&store).len(), told);
    desk.shutdown();
}

#[test]
fn a_new_window_has_its_own_workspace_and_the_persons_shell_in_it_and_closes_with_it() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open: serde_json::Value = serde_json::from_str(TERMINAL).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    let group = crate::projects::key(mine.project.id);
    let theirs = super::super::Call {
        person: true,
        ..mine.call()
    };
    desk.new_window(&theirs, &group).unwrap();
    let windows = store.read(layout::windows).unwrap();
    assert_eq!(windows.len(), 2);
    let (first, second) = (windows[0].id, windows[1].id);
    // Each window shows a workspace of the project; the shell is in the new one's.
    let panes_of = |window: layout::WindowId| {
        store
            .read(|db| {
                let mut panes = vec![];
                for space in layout::workspaces(db, window)? {
                    for tab in layout::tabs(db, space.id)? {
                        panes.extend(tab.tree.panes());
                    }
                }
                Ok(panes)
            })
            .unwrap()
    };
    let row = |key: &str| crate::lock(&desk.ledger).get(key).map(|(row, _)| *row);
    assert_eq!(panes_of(first), [row("t1").unwrap()]);
    assert_eq!(panes_of(second), [row("t2").unwrap()]);
    assert!(windows[1].shown.is_some());
    desk.window_closed(second);
    let windows = store.read(layout::windows).unwrap();
    assert_eq!(
        windows.iter().map(|window| window.id).collect::<Vec<_>>(),
        [first]
    );
    desk.shutdown();
}

#[test]
fn a_restart_gathers_the_other_windows_tabs_into_the_first() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open: serde_json::Value = serde_json::from_str(TERMINAL).unwrap();
    desk.window(&mine.call(), &open).unwrap();
    let group = crate::projects::key(mine.project.id);
    desk.new_window(&mine.call(), &group).unwrap();
    let row = |key: &str| crate::lock(&desk.ledger).get(key).map(|(row, _)| *row);
    let (one, two) = (row("t1").unwrap(), row("t2").unwrap());
    let first = store.read(layout::windows).unwrap()[0].id;
    desk.gather_windows();
    let windows = store.read(layout::windows).unwrap();
    assert_eq!(
        windows.iter().map(|window| window.id).collect::<Vec<_>>(),
        [first]
    );
    assert_eq!(tabs(&store, mine.project.id), [vec![one], vec![two]]);
    desk.shutdown();
}

#[test]
fn a_tab_let_go_over_another_windows_list_moves_into_it_and_away_from_every_list_is_a_window() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open: serde_json::Value = serde_json::from_str(TERMINAL).unwrap();
    for _ in 0..3 {
        desk.window(&mine.call(), &open).unwrap();
    }
    let project = mine.project.id;
    let row = |key: &str| {
        crate::lock(&desk.ledger)
            .get(key)
            .map(|(row, _)| *row)
            .unwrap()
    };
    let (one, two, three) = (row("t1"), row("t2"), row("t3"));
    let windows = || {
        (store.read(layout::windows).unwrap().into_iter())
            .map(|window| window.id)
            .collect::<Vec<_>>()
    };
    let first = windows()[0];
    // Away from every list: a window of its own, with that tab only, its project's shown.
    desk.detached("t3");
    let all = windows();
    assert_eq!(all.len(), 2);
    let second = all[1];
    assert_eq!(tabs_in(&store, first, project), [vec![one], vec![two]]);
    assert_eq!(tabs_in(&store, second, project), [vec![three]]);
    let shown = store
        .read(|db| {
            let window = (layout::windows(db)?.into_iter()).find(|window| window.id == second);
            let Some(space) = window.and_then(|window| window.shown) else {
                return Ok(None);
            };
            Ok(layout::workspace(db, space)?.and_then(|space| space.shown))
        })
        .unwrap();
    assert!(shown.is_some());
    // Over the first window's list, before its second tab: it goes there.
    desk.moved("t3", Some("t2"), Some(first));
    assert_eq!(
        tabs_in(&store, first, project),
        [vec![one], vec![three], vec![two]]
    );
    assert!(tabs_in(&store, second, project).is_empty());
    // Over the second window's list, at its end: it goes back.
    desk.moved("t1", None, Some(second));
    assert_eq!(tabs_in(&store, second, project), [vec![one]]);
    assert_eq!(tabs_in(&store, first, project), [vec![three], vec![two]]);
    desk.shutdown();
}
