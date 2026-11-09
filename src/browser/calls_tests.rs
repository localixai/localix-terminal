//! Tests of `calls`.

use serde_json::json;

use super::*;

#[test]
fn a_reply_with_the_calls_word_answers_it() {
    let calls = Calls::default();
    let call = calls.open();
    assert!(calls.replied(call.id, &call.word, None, Ok(json!("seen"))));
    assert_eq!(call.receiver.try_recv(), Ok(Ok(json!("seen"))));
}

#[test]
fn a_reply_without_the_calls_word_answers_nothing() {
    let calls = Calls::default();
    let call = calls.open();
    assert!(!calls.replied(call.id, "", None, Ok(json!("forged"))));
    assert!(!calls.replied(
        call.id,
        "0123456789abcdef0123456789abcdef",
        None,
        Ok(json!(1))
    ));
    assert!(call.receiver.try_recv().is_err());
    // The call still waits for its own reply.
    assert!(calls.replied(call.id, &call.word, None, Ok(json!("real"))));
}

#[test]
fn another_calls_word_does_not_answer_this_one() {
    let calls = Calls::default();
    let (one, two) = (calls.open(), calls.open());
    assert_ne!(one.id, two.id);
    assert_ne!(one.word, two.word);
    assert!(!calls.replied(one.id, &two.word, None, Ok(json!("crossed"))));
    assert!(one.receiver.try_recv().is_err());
}

#[test]
fn a_reply_from_another_tab_is_refused_where_the_channel_says_which() {
    let calls = Calls::default();
    let call = calls.open();
    calls.aimed(call.id, 3);
    assert!(!calls.replied(call.id, &call.word, Some(4), Ok(json!("other tab"))));
    assert!(call.receiver.try_recv().is_err());
    assert!(calls.replied(call.id, &call.word, Some(3), Ok(json!("its tab"))));
    assert_eq!(call.receiver.try_recv(), Ok(Ok(json!("its tab"))));
}

#[test]
fn a_reply_before_the_call_was_sent_anywhere_names_no_tab() {
    let calls = Calls::default();
    let call = calls.open();
    assert!(!calls.replied(call.id, &call.word, Some(1), Ok(json!(1))));
}

#[test]
fn what_comes_with_a_reply_is_kept_only_for_a_call_that_waits() {
    let calls = Calls::default();
    let call = calls.open();
    let mut kept = 0;
    calls.while_waiting(call.id, "wrong", None, || kept += 1);
    calls.while_waiting(call.id, &call.word, None, || kept += 1);
    calls.forget(call.id);
    calls.while_waiting(call.id, &call.word, None, || kept += 1);
    assert_eq!(kept, 1);
}

#[test]
fn the_native_answer_and_the_window_closing_reach_the_caller() {
    let calls = Calls::default();
    let (one, two) = (calls.open(), calls.open());
    calls.answer(one.id, Err("probe_not_ready".into()));
    assert_eq!(one.receiver.try_recv(), Ok(Err("probe_not_ready".into())));
    calls.fail_all("window_closed");
    assert_eq!(two.receiver.try_recv(), Ok(Err("window_closed".into())));
}
