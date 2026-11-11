//! Tests of `notes`: what counts as a program's note to the person, and how it reads.

use super::*;

fn said(sequences: &[&str]) -> Option<String> {
    let mut notes = Notes::default();
    for sequence in sequences {
        notes.read(sequence);
    }
    notes.waiting().map(|(_, words)| words.to_string())
}

#[test]
fn a_note_is_read_in_either_form_as_one_plain_line() {
    assert_eq!(said(&["9;Build failed"]).as_deref(), Some("Build failed"));
    assert_eq!(
        said(&["777;notify;Agent;Needs  your\tanswer"]).as_deref(),
        Some("Agent: Needs your answer")
    );
    assert_eq!(said(&["777;notify;Done"]).as_deref(), Some("Done"));
    assert_eq!(
        said(&["9;3 files; all changed"]).as_deref(),
        Some("3 files; all changed")
    );
    let long = format!("9;{}", "a".repeat(500));
    assert_eq!(said(&[&long]).unwrap().chars().count(), MOST);
}

#[test]
fn a_request_that_is_not_a_note_says_nothing() {
    // A progress bar, a working folder, a shell's mark, an empty note, another request of 777.
    for other in [
        "9;4;1;50",
        "9;9;/tmp",
        "9;12",
        "133;D;0",
        "9;",
        "9; \x07 ",
        "777;precmd",
        "7;file:///",
    ] {
        assert_eq!(said(&[other]), None, "{other}");
    }
}

#[test]
fn a_note_waits_until_it_is_heard_and_another_is_another_number() {
    let mut notes = Notes::default();
    assert!(!notes.heard());
    notes.read("9;first");
    let (first, _) = notes.waiting().unwrap();
    notes.read("9;second");
    let (second, words) = notes.waiting().unwrap();
    assert_ne!(first, second);
    assert_eq!(words, "second");
    assert!(notes.heard());
    assert_eq!(notes.waiting(), None);
}
