//! Tests of the shell's marks: a command's start and end, each counted once.

use super::Marks;

#[test]
fn a_command_is_running_from_its_start_to_its_end() {
    let mut marks = Marks::default();
    marks.read("133;A");
    assert!(marks.running().is_none(), "a prompt starts no command");
    marks.read("133;C");
    assert!(marks.running().is_some());
    marks.read("133;D;2");
    assert!(marks.running().is_none());
    let (count, ended) = marks.ended();
    assert_eq!(count, 1);
    assert_eq!(ended.unwrap().code, Some(2));
}

#[test]
fn an_end_counts_once_and_only_after_a_start() {
    // Two sets of marks in one shell (its own and the app's) say each thing twice.
    let mut marks = Marks::default();
    for said in [
        "133;A", "133;A", "133;C", "133;C", "133;D;0", "133;D;0", "133;A",
    ] {
        marks.read(said);
    }
    assert_eq!(marks.ended().0, 1);
    // A shell that says an end at every prompt ran nothing when Enter alone was pressed.
    for said in ["133;D;0", "133;A", "133;D;0", "133;A"] {
        marks.read(said);
    }
    assert_eq!(marks.ended().0, 1);
}

#[test]
fn an_end_with_no_code_is_still_an_end() {
    let mut marks = Marks::default();
    marks.read("133;C");
    marks.read("133;D");
    assert_eq!(marks.ended().1.unwrap().code, None);
    marks.read("133;C");
    marks.read("133;D;oops");
    assert_eq!(marks.ended().1.unwrap().code, None);
    marks.read("133;C;extra");
    marks.read("133;D;130;aid=7");
    assert_eq!(marks.ended(), (3, marks.ended().1));
    assert_eq!(marks.ended().1.unwrap().code, Some(130));
}

#[test]
fn other_strings_are_not_marks() {
    let mut marks = Marks::default();
    for said in [
        "0;a title",
        "133;Z",
        "133",
        "1337;File=x",
        "8;;http://x",
        "",
    ] {
        marks.read(said);
    }
    assert!(marks.running().is_none());
    assert_eq!(marks.ended(), (0, None));
}
