//! Tests of `host`.

use super::*;

#[test]
fn a_message_is_the_token_then_the_json() {
    let sent = unsealed("tok", "tok\n{\"id\":3,\"ok\":true}").unwrap();
    assert_eq!(sent["id"], 3);
}

#[test]
fn a_message_without_the_token_in_front_is_ignored() {
    // The old form, with the token inside the JSON, is what a page could read back.
    assert!(unsealed("tok", "{\"t\":\"tok\",\"id\":3}").is_none());
    assert!(unsealed("tok", "other\n{\"id\":3}").is_none());
    assert!(unsealed("tok", "tok{\"id\":3}").is_none());
}

#[test]
fn json_with_half_an_emoji_does_not_parse() {
    // Why the probe makes every string well formed before it sends: the reply would be dropped.
    assert!(unsealed("tok", "tok\n{\"a\":\"x\\ud83d\"}").is_none());
    assert!(unsealed("tok", "tok\n{\"a\":\"x\\ud83d\\ude00\"}").is_some());
}

#[test]
fn a_title_is_cut_at_a_character_not_in_the_middle_of_one() {
    assert_eq!(clipped("A page"), "A page");
    let long = "é".repeat(TITLE + 50);
    let cut = clipped(&long);
    assert_eq!(cut.chars().count(), TITLE);
    assert!(long.starts_with(&cut));
    let exact = "x".repeat(TITLE);
    assert_eq!(clipped(&exact), exact);
}
