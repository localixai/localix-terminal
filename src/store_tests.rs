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
fn existing_migrations_are_unchanged_and_new_ones_are_appended() {
    use std::fmt::Write as _;

    use sha2::{Digest, Sha256};
    let kept = [
        "c5d5af310c49a3035aebef536c126ba7379674cb6384dd0ec371bb31353165ff",
        "a88f7fa952d7f8a33517e83f27a06d4669784f2abf9938d8a2439324155d565e",
    ];
    assert!(
        STEPS.len() >= kept.len(),
        "existing migrations cannot be removed"
    );
    for (at, expected) in kept.iter().enumerate() {
        let mut actual = String::new();
        for byte in Sha256::digest(STEPS[at].as_bytes()) {
            write!(&mut actual, "{byte:02x}").unwrap();
        }
        assert_eq!(
            actual,
            *expected,
            "migration {} changed: append a new migration instead",
            at + 1
        );
    }
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

#[test]
fn a_database_and_the_directory_made_for_it_are_the_owners_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new();
    let dir = scratch.path().join("data");
    let path = dir.join("desk.db");
    drop(Store::open(&path).unwrap());
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&path), 0o600);
    // A directory that was there is not the app's to change.
    let shared = scratch.path().join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();
    drop(Store::open(&shared.join("desk.db")).unwrap());
    assert_eq!(mode(&shared), 0o755);
    assert_eq!(mode(&shared.join("desk.db")), 0o600);
}

#[test]
fn a_store_in_memory_in_place_of_the_database_says_why() {
    assert_eq!(Store::memory().unwrap().kept_in_memory(), None);
    let instead = Store::memory_instead("read-only").unwrap();
    assert_eq!(instead.kept_in_memory(), Some("read-only"));
}

#[test]
fn processes_that_open_a_new_database_at_once_each_open_it() {
    // The app and the CLI open the same file as the app starts: neither may fail for the
    // other's hold on it, nor run a step of the schema the other ran.
    for _ in 0..10 {
        let scratch = Scratch::new();
        let path = scratch.path().join("desk.db");
        let start = std::sync::Arc::new(std::sync::Barrier::new(8));
        let opening: Vec<_> = (0..8)
            .map(|_| {
                let (path, start) = (path.clone(), std::sync::Arc::clone(&start));
                std::thread::Builder::new()
                    .name("opening".into())
                    .spawn(move || {
                        start.wait();
                        Store::open(&path).map(|store| steps(&store))
                    })
                    .unwrap()
            })
            .collect();
        for opened in opening {
            assert_eq!(opened.join().unwrap(), Ok(STEPS.len()));
        }
    }
}
