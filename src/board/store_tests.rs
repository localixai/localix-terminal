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

#[test]
fn a_project_whose_folder_is_gone_is_not_brought_back_by_its_board() {
    let scratch = Scratch::new();
    let gone = scratch.path().join("moved-away");
    export(&gone, &Board::default()).unwrap();
    assert!(!gone.exists());
}

#[test]
fn an_export_leaves_no_temporary_file_and_does_not_rewrite_what_it_already_says() {
    let scratch = Scratch::new();
    let mut board = Board::default();
    board.add("one", "", None, "agent").unwrap();
    export(scratch.path(), &board).unwrap();
    let path = place(scratch.path());
    let first = std::fs::metadata(&path).unwrap().modified().unwrap();
    export(scratch.path(), &board).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["board.json"]);
}

#[cfg(unix)]
#[test]
fn an_export_does_not_write_through_a_localix_that_leads_out_of_the_project() {
    let (project, elsewhere) = (Scratch::new(), Scratch::new());
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join(".localix")).unwrap();
    let refused = export(project.path(), &Board::default()).unwrap_err();
    assert!(refused.contains("leaves the project"), "{refused}");
    assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0);
    // One that stays inside the project is the person's own choice.
    let project = Scratch::new();
    std::fs::create_dir_all(project.path().join("kept")).unwrap();
    std::os::unix::fs::symlink("kept", project.path().join(".localix")).unwrap();
    export(project.path(), &Board::default()).unwrap();
    assert!(project.path().join("kept/board.json").is_file());
}

#[test]
fn a_board_read_from_a_file_never_reuses_an_id_it_holds() {
    let scratch = Scratch::new();
    scratch.file(
        ".localix/board.json",
        r#"{"next": 1, "limit": 1, "cards": [
            {"id": "c1", "title": "a", "column": "todo"},
            {"id": "c9", "title": "b", "column": "todo"},
            {"id": "other", "title": "c", "column": "todo"}]}"#,
    );
    let mut board = written(scratch.path()).unwrap().unwrap();
    assert_eq!(board.next, 10);
    assert_eq!(board.add("new", "", None, "agent").unwrap(), "c10");
}
