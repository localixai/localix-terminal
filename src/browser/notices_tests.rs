//! Tests of `notices`.

use serde_json::json;

use super::{Notice, Notices};

fn confirm(answer: &str) -> serde_json::Value {
    json!({"kind": "confirm", "text": "Delete it?", "answer": answer})
}

#[test]
fn a_reply_s_notices_are_kept_for_the_tab_of_the_call_that_waited_for_it() {
    let notices = Notices::default();
    let mark = notices.mark();
    notices.arrived(
        7,
        &json!([confirm("Cancel"), {"kind": "said", "text": "Saved"}]),
    );
    notices.file(7, 2);
    let told = notices.since(2, mark);
    assert_eq!(told.len(), 2);
    assert!(told[0].declined());
    assert_eq!(
        told[0].to_string(),
        "a confirm: \"Delete it?\", answered Cancel"
    );
    assert_eq!(told[1].to_string(), "announced: \"Saved\"");
    assert!(
        notices.since(3, mark).is_empty(),
        "another tab heard nothing"
    );
}

#[test]
fn a_call_reads_only_what_came_after_its_mark() {
    let notices = Notices::default();
    notices.arrived(1, &json!([{"kind": "alert", "text": "old"}]));
    notices.file(1, 1);
    let mark = notices.mark();
    notices.arrived(2, &json!([confirm("OK")]));
    notices.file(2, 1);
    let told = notices.since(1, mark);
    assert_eq!(told.len(), 1);
    assert!(!told[0].declined());
}

#[test]
fn what_no_call_files_is_never_kept_and_a_closed_tab_forgets() {
    let notices = Notices::default();
    notices.file(9, 1);
    notices.arrived(3, &json!("not a list"));
    notices.file(3, 1);
    assert!(notices.since(1, 0).is_empty());
    notices.arrived(4, &json!([{"kind": "alert", "text": "hi"}]));
    notices.file(4, 1);
    notices.arm(1, Some(super::Armed { text: None }));
    notices.forget(1);
    assert!(notices.since(1, 0).is_empty());
    assert_eq!(notices.armed(1), None);
}

#[test]
fn a_field_notice_reads_as_its_words() {
    let field = Notice {
        kind: "field".into(),
        text: "Phone kept \"555\" of the typed \"5551234\"".into(),
        answer: None,
    };
    assert_eq!(
        field.to_string(),
        "Phone kept \"555\" of the typed \"5551234\""
    );
}
