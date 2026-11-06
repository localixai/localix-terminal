//! Tests of `store`: a board's row in the database, the file a project kept it in before, and
//! the file it is written out to.

use super::*;
use crate::store::Store;
use crate::testing::Scratch;

#[test]
fn a_project_writes_its_board_out_in_its_folder() {
    let scratch = Scratch::new();
    assert_eq!(
        place(scratch.path()),
        scratch.path().join(".localix/board.json")
    );
}

#[test]
fn a_board_is_kept_as_its_projects_row_and_read_back_whole() {
    let store = Store::memory().unwrap();
    let project = store
        .write(|tx| crate::projects::find_or_make(tx, Path::new("/work/shop"), 1))
        .unwrap();
    assert_eq!(store.read(|db| load(db, project.id)).unwrap(), None);
    let mut board = Board::default();
    board.add("one", "why", None, "agent").unwrap();
    store.write(|tx| save(tx, project.id, &board)).unwrap();
    board.add("two", "", None, "agent").unwrap();
    store.write(|tx| save(tx, project.id, &board)).unwrap();
    assert_eq!(store.read(|db| load(db, project.id)).unwrap(), Some(board));
    assert_eq!(
        store.read(projects).unwrap(),
        [(project.id, PathBuf::from("/work/shop"))]
    );
    // A project forgotten takes its board with it.
    store
        .write(|tx| crate::projects::forget(tx, project.id))
        .unwrap();
    assert_eq!(store.read(projects).unwrap(), []);
}

#[test]
fn the_file_written_out_reads_as_the_board_a_project_kept_before() {
    let scratch = Scratch::new();
    assert_eq!(written(scratch.path()).unwrap(), None);
    let mut board = Board::default();
    board.add("one", "why", None, "agent").unwrap();
    export(scratch.path(), &board).unwrap();
    assert_eq!(written(scratch.path()).unwrap(), Some(board));
}

#[test]
fn a_file_that_is_not_a_board_is_refused_and_left_as_it_is() {
    let scratch = Scratch::new();
    let path = scratch.file(".localix/board.json", "not a board");
    let refused = written(scratch.path()).unwrap_err();
    assert!(refused.contains("does not read as a board"), "{refused}");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "not a board");
}
