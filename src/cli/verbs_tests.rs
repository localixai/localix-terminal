//! Tests of the short commands' own options, and of the tab keys they tell from refs.

use super::own;

fn words(given: &[&str]) -> Vec<String> {
    given.iter().map(|word| (*word).to_string()).collect()
}

#[test]
fn the_callers_options_are_taken_wherever_they_are_before_two_dashes() {
    let given = own(&words(&[
        "--as",
        "cart",
        "click",
        "e1",
        "--project",
        "shop",
    ]))
    .unwrap();
    assert_eq!(given.agent.as_deref(), Some("cart"));
    assert_eq!(given.project, Some("shop".into()));
    assert_eq!(given.rest, words(&["click", "e1"]));
    // After `--` they are text.
    let given = own(&words(&["type", "t1", "--", "--as", "x"])).unwrap();
    assert_eq!(given.agent, None);
    assert_eq!(given.rest, words(&["type", "t1", "--", "--as", "x"]));
    assert!(own(&words(&["click", "e1", "--as"])).is_err());
}

#[test]
fn no_kind_of_tab_has_the_letter_of_a_ref() {
    // A ref is `e12`: a tab's key on `e` would read as one, and the other way round.
    let letters: Vec<char> = (super::super::kinds().iter().map(|about| about.letter))
        .chain(['w', 't', 'b', 'f'])
        .collect();
    assert!(!letters.contains(&'e'), "{letters:?}");
}
