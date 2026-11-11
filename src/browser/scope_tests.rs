//! Tests of `scope`.

use super::Scope;

#[test]
fn the_first_tab_shown_names_the_project_and_another_projects_stay_out_of_sight() {
    let mut scope = Scope::default();
    assert!(scope.in_sight("1") && scope.in_sight("2"), "none shown yet");
    scope.showed("1", "w1");
    assert_eq!(scope.shown(), "1");
    assert!(scope.in_sight("1") && !scope.in_sight("2"));
    scope.touched("2");
    scope.touched("1");
    assert!(scope.marked("2") && !scope.marked("1"));
}

#[test]
fn going_to_a_project_shows_its_last_tab_and_clears_its_mark() {
    let mut scope = Scope::default();
    scope.showed("2", "t3");
    scope.showed("1", "w1");
    scope.touched("2");
    let open = ["w2".to_string(), "t3".to_string()];
    assert_eq!(scope.go("2", &open).as_deref(), Some("t3"));
    assert!(!scope.marked("2") && scope.shown() == "2");
    // Its last tab closed: its first; none open: none.
    assert_eq!(scope.go("1", &["w4".to_string()]).as_deref(), Some("w4"));
    assert_eq!(scope.go("3", &[]), None);
    assert_eq!(scope.shown(), "3");
    // Till a tab of it shows, the window waits for the app to open what it had.
    assert!(scope.awaiting());
    scope.showed("3", "t5");
    assert!(!scope.awaiting());
}

#[test]
fn the_project_of_the_tab_opening_is_taken_once() {
    let mut scope = Scope::default();
    scope.opening("2", false);
    assert_eq!(scope.take_opening(), Some(("2".to_string(), false)));
    assert_eq!(scope.take_opening(), None);
}

#[test]
fn a_tab_that_opens_beside_another_is_said_so_once() {
    let mut scope = Scope::default();
    // A page: said with its project, as it has no key yet.
    scope.opening("2", true);
    assert!(scope.opens_aside());
    assert_eq!(scope.take_opening(), Some(("2".to_string(), true)));
    // A terminal: by its key.
    scope.aside("t4");
    assert!(!scope.take_aside("t5"));
    assert!(scope.take_aside("t4"));
    assert!(!scope.take_aside("t4"));
}
