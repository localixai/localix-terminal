//! Tests of `lines`: the line each record of a program's state has for the person, kept in place
//! while the program flips and let go of once the person typed and it went on.

use base64::Engine;

use super::super::{Doing, Records, Said, read};

fn b64(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text)
}

/// The records after `bodies`, applied in turn.
fn after(bodies: &[&str]) -> Records {
    let mut records = Records::default();
    for body in bodies {
        if let Some(Said::Report(report)) = read(body) {
            records.apply(report);
        }
    }
    records
}

/// The lines of `records`, as the Event Center reads them: id, state and round.
fn lines(records: &Records) -> Vec<(String, Doing, u64)> {
    (records.lines().into_iter())
        .map(|line| (line.id, line.state, line.round))
        .collect()
}

#[test]
fn a_record_flipping_its_state_keeps_one_line_until_the_person_types() {
    let mut records = after(&["state=blocked:app=claude-code"]);
    let first = lines(&records);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].1, Doing::Blocked);
    let lined = records.lined();
    // At work and waiting again, over and over, with no one at the terminal: the one line as
    // it was, and nothing new to tell.
    for _ in 0..50 {
        for body in [
            "state=working:app=claude-code",
            "state=blocked:app=claude-code",
        ] {
            if let Some(Said::Report(report)) = read(body) {
                records.apply(report);
            }
        }
    }
    assert_eq!(lines(&records), first);
    assert_eq!(records.lined(), lined, "no line changed");
    // At work, the line stays while no one has looked: what waited is still to be seen.
    if let Some(Said::Report(report)) = read("state=working:app=claude-code") {
        records.apply(report);
    }
    assert_eq!(lines(&records), first);
    // The person typed: the line goes once the program goes on, and what waits after is a
    // line of its own.
    records.seen();
    assert!(records.lines().is_empty());
    if let Some(Said::Report(report)) = read("state=blocked:app=claude-code") {
        records.apply(report);
    }
    let again = lines(&records);
    assert_eq!(again.len(), 1);
    assert_ne!(again[0].2, first[0].2, "a new round");
}

#[test]
fn a_record_still_waiting_when_the_person_types_is_a_new_line_when_it_asks_again() {
    let mut records = after(&[&format!("state=blocked:msg={}", b64("first?"))]);
    let first = lines(&records)[0].2;
    records.seen();
    // Still waiting: its line stays.
    assert_eq!(lines(&records)[0].2, first);
    if let Some(Said::Report(report)) = read(&format!("state=blocked:msg={}", b64("second?"))) {
        records.apply(report);
    }
    let line = &records.lines()[0];
    assert_ne!(line.round, first);
    assert_eq!(line.msg.as_deref(), Some("second?"));
}
