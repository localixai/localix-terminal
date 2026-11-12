//! Tests of `catalog::index::schema`: a file of another version, or none at all, made again.

use super::*;
use crate::catalog::Index;
use crate::catalog::index::tests::{batch, listing};
use crate::testing::Scratch;

#[test]
fn the_bundled_sqlite_has_fts5() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE VIRTUAL TABLE t USING fts5(a); INSERT INTO t VALUES ('x');")
        .unwrap();
}

#[test]
fn a_file_of_this_version_is_kept_and_one_of_another_is_made_again() {
    let scratch = Scratch::new();
    let path = scratch.path().join("data/catalog.sqlite");
    let index = Index::open(&path).unwrap();
    index
        .put(
            "skills",
            3,
            &batch(vec![listing("skills", "pdf", "PDF", "x")], "1"),
            1,
        )
        .unwrap();
    drop(index);
    assert!(Index::open(&path).unwrap().get("skills/pdf").is_some());
    let older = Connection::open(&path).unwrap();
    older.execute_batch("PRAGMA user_version = 99").unwrap();
    drop(older);
    let index = Index::open(&path).unwrap();
    assert_eq!(index.get("skills/pdf"), None);
    assert_eq!(index.mark("skills"), None);
}

#[test]
fn a_file_that_is_no_database_or_another_database_is_made_again() {
    let scratch = Scratch::new();
    let junk = scratch.file(
        "junk.sqlite",
        "not a database at all, but long enough to be read",
    );
    assert!(
        Index::open(&junk)
            .unwrap()
            .counts()
            .iter()
            .all(|(_, n)| *n == 0)
    );
    let other = scratch.path().join("other.sqlite");
    let made = Connection::open(&other).unwrap();
    made.execute_batch("CREATE TABLE someone_else (x)").unwrap();
    drop(made);
    assert!(Index::open(&other).is_ok());
}

#[test]
fn a_file_of_the_version_before_origins_is_made_again_with_them() {
    let scratch = Scratch::new();
    let path = scratch.path().join("catalog.sqlite");
    let older = Connection::open(&path).unwrap();
    older
        .execute_batch(
            "CREATE TABLE listings (n INTEGER PRIMARY KEY, id TEXT NOT NULL UNIQUE);
             INSERT INTO listings (id) VALUES ('skills/old');
             CREATE TABLE marks (source TEXT PRIMARY KEY, mark TEXT, read_at INTEGER, more INTEGER);
             INSERT INTO marks VALUES ('skills', 'e0', 1, 0);
             PRAGMA user_version = 1;",
        )
        .unwrap();
    drop(older);
    let index = Index::open(&path).unwrap();
    assert_eq!(index.get("skills/old"), None);
    assert_eq!(index.mark("skills"), None);
    let mut pdf = listing("skills", "pdf", "PDF", "x");
    pdf.origin = "github.com/anthropics/skills#skills/pdf".into();
    index
        .put("skills", 3, &batch(vec![pdf.clone()], "1"), 1)
        .unwrap();
    assert_eq!(index.get("skills/pdf"), Some(pdf));
    let version: i64 = Connection::open(&path)
        .unwrap()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, VERSION);
}
