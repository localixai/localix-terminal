//! A question's answer reaches whoever waits for it once, however often the system calls back,
//! reaches every ask of it made while it is up, and is read from the button the alert says.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{answer_of, once};
use crate::browser::dialog::Answer;

#[test]
fn an_answer_reaches_its_asker_once() {
    let heard = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&heard);
    let answer = once(move |response: isize| {
        counted.fetch_add(usize::try_from(response).unwrap_or(0), Ordering::Relaxed);
    });
    answer(1000);
    answer(1001);
    assert_eq!(heard.load(Ordering::Relaxed), 1000);
}

#[test]
fn a_sheet_hangs_only_from_a_window_on_screen() {
    assert!(super::hangs(true, false));
    assert!(
        !super::hangs(false, false),
        "a window put away shows no sheet"
    );
    assert!(!super::hangs(true, true), "nor one in the Dock");
}

#[test]
fn a_question_asked_again_while_it_is_up_waits_for_the_one_shown() {
    let heard = Arc::new(std::sync::Mutex::new(Vec::new()));
    let hear = |who: &'static str| {
        let heard = Arc::clone(&heard);
        Box::new(move |answer: Answer| crate::lock(&heard).push((who, answer)))
            as Box<dyn FnOnce(Answer)>
    };
    let mut up = super::Up::default();
    assert!(up.join("quit", hear("first")), "the first ask is shown");
    assert!(
        !up.join("quit", hear("again")),
        "the same one again joins it"
    );
    assert!(
        up.join("other", hear("other")),
        "another question is its own"
    );
    for done in up.answered(&"quit") {
        done(Answer::Yes);
    }
    assert_eq!(
        *crate::lock(&heard),
        [("first", Answer::Yes), ("again", Answer::Yes)]
    );
    assert!(up.join("quit", hear("later")), "answered, it is asked anew");
    assert_eq!(up.answered(&"other").len(), 1);
}

#[test]
fn the_alerts_buttons_are_its_act_cancel_and_its_second_act() {
    assert_eq!(
        answer_of(1000),
        Answer::Yes,
        "the first button added, the default"
    );
    assert_eq!(answer_of(1001), Answer::No, "Cancel, Escape");
    assert_eq!(answer_of(1002), Answer::Other, "the third, a second act");
    assert_eq!(
        answer_of(-1000),
        Answer::No,
        "a sheet ended otherwise does nothing"
    );
}
