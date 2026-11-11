//! The doctor's log: what a round says that the one before did not.

use super::news;

fn lines(said: &[&str]) -> Vec<String> {
    said.iter().map(ToString::to_string).collect()
}

#[test]
fn the_first_round_is_said_whole_and_a_round_like_the_last_says_nothing() {
    let round = lines(&["the command is linked already", "Codex (not added): added"]);
    assert_eq!(news(&[], &round), round);
    assert!(news(&round, &round).is_empty());
}

#[test]
fn only_what_changed_since_the_last_round_is_said() {
    let before = lines(&["the command is linked already"]);
    let now = lines(&[
        "the command is linked already",
        "Codex (another program is named there): added",
    ]);
    assert_eq!(
        news(&before, &now),
        lines(&["Codex (another program is named there): added"])
    );
}
