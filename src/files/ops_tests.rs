//! Tests of `files::ops`: a file and a folder made where the person asked, one renamed or moved
//! into another folder, and nothing ever written over what is there.

use super::{Refused, make, move_into, named, rename};
use crate::testing::Scratch;

#[test]
fn a_name_is_one_a_folder_can_hold() {
    for good in ["main.rs", ".env", "a b", "x.tar.gz", "папка"] {
        assert!(named(good), "{good}");
    }
    for bad in ["", ".", "..", "a/b", "a\nb", "a\0b", " ", &"x".repeat(256)] {
        assert!(!named(bad), "{bad:?}");
    }
}

#[test]
fn a_file_and_a_folder_are_made_in_the_folder_named_and_say_their_path() {
    let dir = Scratch::new();
    dir.file("src/main.rs", "");
    assert_eq!(
        make(dir.path(), "src", "lib.rs", false).unwrap(),
        "src/lib.rs"
    );
    assert_eq!(make(dir.path(), "", "docs", true).unwrap(), "docs");
    assert!(dir.path().join("src/lib.rs").is_file());
    assert!(dir.path().join("docs").is_dir());
}

#[test]
fn nothing_is_made_over_what_is_there_or_under_a_name_no_file_has() {
    let dir = Scratch::new();
    dir.file("notes.md", "kept");
    let taken = make(dir.path(), "", "notes.md", false);
    assert_eq!(taken, Err(Refused::Taken("notes.md".into())));
    assert_eq!(make(dir.path(), "", "notes.md", true), taken);
    assert_eq!(make(dir.path(), "", "a/b", false), Err(Refused::Name));
    assert_eq!(make(dir.path(), "gone", "a", false), Err(Refused::Gone));
    assert_eq!(make(dir.path(), "../out", "a", false), Err(Refused::Gone));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("notes.md")).unwrap(),
        "kept"
    );
}

#[test]
fn a_rename_keeps_the_folder_and_never_takes_a_name_in_use() {
    let dir = Scratch::new();
    dir.file("src/old.rs", "one");
    dir.file("src/other.rs", "two");
    assert_eq!(
        rename(dir.path(), "src/old.rs", "new.rs").unwrap(),
        "src/new.rs"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/new.rs")).unwrap(),
        "one"
    );
    assert!(!dir.path().join("src/old.rs").exists());
    let taken = rename(dir.path(), "src/new.rs", "other.rs");
    assert_eq!(taken, Err(Refused::Taken("other.rs".into())));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/other.rs")).unwrap(),
        "two"
    );
    assert_eq!(
        rename(dir.path(), "src/new.rs", "new.rs").unwrap(),
        "src/new.rs"
    );
    assert_eq!(rename(dir.path(), "src/new.rs", ".."), Err(Refused::Name));
    assert_eq!(rename(dir.path(), "src/none.rs", "x"), Err(Refused::Gone));
    assert_eq!(rename(dir.path(), "", "x"), Err(Refused::Gone));
}

#[test]
fn a_rename_that_changes_only_the_letters_case_is_a_rename() {
    let dir = Scratch::new();
    dir.file("readme.md", "text");
    assert_eq!(
        rename(dir.path(), "readme.md", "README.md").unwrap(),
        "README.md"
    );
    let names: Vec<String> = (std::fs::read_dir(dir.path()).unwrap())
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["README.md"]);
}

#[test]
fn an_entry_moves_into_another_folder_under_its_own_name() {
    let dir = Scratch::new();
    dir.file("a/one.txt", "1");
    dir.file("b/keep.txt", "");
    assert_eq!(
        move_into(dir.path(), "a/one.txt", "b").unwrap(),
        "b/one.txt"
    );
    assert_eq!(move_into(dir.path(), "b/one.txt", "").unwrap(), "one.txt");
    assert_eq!(move_into(dir.path(), "b", "a").unwrap(), "a/b");
    assert!(dir.path().join("a/b/keep.txt").is_file());
    assert!(dir.path().join("one.txt").is_file());
}

#[test]
fn a_move_goes_nowhere_it_cannot() {
    let dir = Scratch::new();
    dir.file("a/one.txt", "mine");
    dir.file("b/one.txt", "theirs");
    dir.file("a/in/deep.txt", "");
    // Where it already is: nothing to do, and no refusal.
    assert_eq!(
        move_into(dir.path(), "a/one.txt", "a").unwrap(),
        "a/one.txt"
    );
    let taken = move_into(dir.path(), "a/one.txt", "b");
    assert_eq!(taken, Err(Refused::Taken("one.txt".into())));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b/one.txt")).unwrap(),
        "theirs"
    );
    assert_eq!(move_into(dir.path(), "a", "a"), Err(Refused::Inside));
    assert_eq!(move_into(dir.path(), "a", "a/in"), Err(Refused::Inside));
    assert_eq!(
        move_into(dir.path(), "a/one.txt", "b/one.txt"),
        Err(Refused::Gone)
    );
    assert_eq!(move_into(dir.path(), "", "b"), Err(Refused::Gone));
}

#[test]
fn a_refusal_says_what_happened_in_plain_words() {
    assert_eq!(
        Refused::Taken("a.rs".into()).said(),
        "The name “a.rs” is already taken."
    );
    let failed = Refused::from(std::io::Error::from_raw_os_error(13));
    assert_eq!(failed.said(), "Permission denied.");
}
