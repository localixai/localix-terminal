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
