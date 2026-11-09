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

#[test]
fn a_load_with_the_window_closed_is_refused_at_once() {
    let shared = events::fresh();
    let (reply, answer) = std::sync::mpsc::sync_channel(1);
    let url = "https://a.example/".to_string();
    web::closed(
        &Cmd::Navigate {
            url,
            tab: Some(3),
            reply,
        },
        &shared,
    );
    assert_eq!(answer.try_recv(), Ok(Err(WINDOW_CLOSED.to_string())));
}

#[test]
fn a_load_in_a_tab_closed_meanwhile_is_refused_at_once() {
    let (reply, answer) = std::sync::mpsc::sync_channel(1);
    web::load(None, "https://a.example/", Some(3), &reply);
    let refused = answer.try_recv().unwrap().unwrap_err();
    assert!(refused.starts_with("tab 3 is closed"), "{refused}");
}

#[test]
fn a_tab_gone_leaves_no_project_or_detail_behind() {
    let shared = events::fresh();
    for key in ["w3", "w4", "t3"] {
        crate::lock(&shared.groups).insert(key.into(), "proj".into());
        crate::lock(&shared.details).insert(key.into(), "make".into());
    }
    shared.forget(&[Key::Web(3)]);
    let groups = crate::lock(&shared.groups);
    assert!(!groups.contains_key("w3") && !crate::lock(&shared.details).contains_key("w3"));
    // Another page, and a terminal of the same number, are kept.
    assert!(groups.contains_key("w4") && groups.contains_key("t3"));
}
