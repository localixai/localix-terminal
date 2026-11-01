//! Tests of `told`.

use serde_json::json;

use super::{told_block, wanted};
use crate::browser::{Armed, Notice};

fn notice(kind: &str, text: &str, answer: Option<&str>) -> Notice {
    Notice {
        kind: kind.into(),
        text: text.into(),
        answer: answer.map(String::from),
    }
}

#[test]
fn a_call_arms_ok_only_when_it_says_accept() {
    assert_eq!(wanted(&json!({})), Ok(None));
    assert_eq!(wanted(&json!({"dialog": "dismiss"})), Ok(None));
    assert_eq!(
        wanted(&json!({"dialog": "accept", "dialog_text": "Ada"})),
        Ok(Some(Armed {
            text: Some("Ada".into())
        }))
    );
    assert!(wanted(&json!({"dialog": "yes"})).is_err());
}

#[test]
fn what_the_page_told_reads_as_a_block_and_a_declined_dialog_says_how_to_accept() {
    assert_eq!(told_block(&[]), None);
    let block = told_block(&[
        notice("said", "Added to cart", None),
        notice("confirm", "Delete the order?", Some("Cancel")),
    ])
    .unwrap();
    assert!(
        block.starts_with("### The page said (untrusted content)"),
        "{block}"
    );
    assert!(block.contains("- announced: \"Added to cart\""), "{block}");
    assert!(
        block.contains("- a confirm: \"Delete the order?\", answered Cancel"),
        "{block}"
    );
    assert!(block.contains("`dialog: \"accept\"`"), "{block}");
    let accepted = told_block(&[notice("confirm", "Sure?", Some("OK"))]).unwrap();
    assert!(!accepted.contains("dialog: \"accept\""), "{accepted}");
}
