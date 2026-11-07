//! Tests of `store`: the schema brought up to date once, a write kept whole or not at all, and a
//! database a newer build wrote left as it is.

use super::*;
use crate::testing::Scratch;

fn steps(store: &Store) -> usize {
    store
        .read(|db| {
            db.pragma_query_value(None, "user_version", |row| row.get(0))
                .map_err(|why| why.to_string())
        })
        .unwrap()
}

#[test]
fn a_database_is_made_with_every_step_and_opened_again_as_it_was() {
    let scratch = Scratch::new();
    let path = scratch.path().join("deep").join("desk.db");
    let store = Store::open(&path).unwrap();
    assert_eq!(steps(&store), STEPS.len());
    store
        .write(|tx| {
            tx.execute("INSERT INTO state (key, value) VALUES ('me', '{}')", [])
                .map(drop)
                .map_err(|why| why.to_string())
        })
        .unwrap();
    drop(store);

    let again = Store::open(&path).unwrap();
    assert_eq!(steps(&again), STEPS.len());
    let kept: String = again
        .read(|db| {
            db.query_row("SELECT value FROM state WHERE key = 'me'", [], |row| {
                row.get(0)
            })
            .map_err(|why| why.to_string())
        })
        .unwrap();
    assert_eq!(kept, "{}");
}

#[test]
fn a_write_that_fails_keeps_nothing_of_itself() {
    let store = Store::memory().unwrap();
    let failed: Result<(), String> = store.write(|tx| {
        tx.execute("INSERT INTO state (key, value) VALUES ('me', '{}')", [])
            .map_err(|why| why.to_string())?;
        Err("refused".to_string())
    });
    assert_eq!(failed, Err("refused".to_string()));
    let count: u64 = store
        .read(|db| {
            db.query_row("SELECT COUNT(*) FROM state", [], |row| row.get(0))
                .map_err(|why| why.to_string())
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn a_database_a_newer_build_wrote_is_left_as_it_is() {
    let scratch = Scratch::new();
    let path = scratch.path().join("desk.db");
    drop(Store::open(&path).unwrap());
    let newer = Connection::open(&path).unwrap();
    newer
        .pragma_update(None, "user_version", STEPS.len() + 1)
        .unwrap();
    drop(newer);
    let refused = Store::open(&path).err().unwrap_or_default();
    assert!(refused.contains("newer llx-desk"), "{refused}");
}

#[test]
fn the_tabs_of_a_database_before_windows_become_panes_each_a_tab_and_a_split_one_tab() {
    let mut connection = Connection::open_in_memory().unwrap();
    let tx = connection.transaction().unwrap();
    tx.execute_batch(STEPS[0]).unwrap();
    tx.execute_batch(
        "INSERT INTO projects (id, path, name, created, opened) VALUES (1, '/a', 'a', 0, 5),
             (2, '/b', 'b', 0, 9);
         INSERT INTO tabs (id, project, kind, title, place, opened, shown, closed) VALUES
             (10, 1, 'page', 'one', 1, 1, 3, NULL),
             (11, 1, 'terminal', 'two', 2, 1, 7, NULL),
             (12, 1, 'page', 'gone', 3, 1, NULL, 4),
             (13, 2, 'board', 'board', 1, 2, 1, NULL),
             (14, 1, 'terminal', 'three', 4, 1, 1, NULL);
         INSERT INTO visits (project, tab, url, title, at) VALUES (1, 10, 'u', 't', 1);
         INSERT INTO state (key, value) VALUES
             ('split 1', '{\"shown\":11,\"beside\":14,\"right\":true}');",
    )
    .unwrap();
    tx.pragma_update(None, "user_version", 1).unwrap();
    tx.commit().unwrap();
    let store = Store::ready(connection).unwrap();
    assert_eq!(steps(&store), STEPS.len());
    let read = |sql: &str| -> Vec<String> {
        store
            .read(|db| {
                let mut query = db.prepare(sql).map_err(|why| why.to_string())?;
                let rows = query
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(|why| why.to_string())?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|why| why.to_string())?;
                Ok(rows)
            })
            .unwrap()
    };
    // The panes are what the tabs were, history and pages with them.
    assert_eq!(
        read("SELECT title FROM panes ORDER BY id"),
        ["one", "two", "gone", "board", "three"]
    );
    assert_eq!(read("SELECT url FROM visits WHERE pane = 10"), ["u"]);
    // One window, a workspace a project, a tab an open pane, the split's two panes one tab.
    assert_eq!(read("SELECT id || ':' || shown FROM windows"), ["1:2"]);
    assert_eq!(
        read("SELECT project || ':' || shown FROM workspaces ORDER BY project"),
        ["1:11", "2:13"]
    );
    assert_eq!(
        read("SELECT id || ':' || place FROM tabs ORDER BY id"),
        ["10:1", "11:2", "13:1"]
    );
    let tree = store
        .read(|db| crate::layout::tab(db, 11))
        .unwrap()
        .unwrap();
    assert_eq!(tree.tree.panes(), [11, 14]);
    assert_eq!(tree.focused, Some(11));
    assert!(read("SELECT key FROM state").is_empty());
}
