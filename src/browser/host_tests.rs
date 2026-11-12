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

/// A page's message, sealed with the run's token as the probe seals it.
fn sealed(shared: &Shared, json: &str) -> String {
    format!("{}\n{json}", shared.token)
}

#[test]
fn a_comment_is_dropped_while_the_person_is_not_commenting() {
    let shared = events::fresh();
    let fb = sealed(
        &shared,
        r##"{"fb":{"element":{"selector":"#a"},"note":"do it","n":1}}"##,
    );
    assert!(
        shared.ipc(&fb, None).is_none(),
        "nobody turned commenting on"
    );
    shared.turn.commenting(true);
    assert!(matches!(shared.ipc(&fb, None), Some(Said::Noted(_))));
}

#[test]
fn a_reply_that_names_the_call_but_not_its_word_answers_nothing() {
    let shared = events::fresh();
    let call = shared.calls.open();
    let id = call.id;
    let guessed = format!(r#"{{"id":"{id}","ok":"forged"}}"#);
    shared.ipc(&sealed(&shared, &guessed), None);
    let wrong = format!(r#"{{"id":"{id}","word":"0000","ok":"forged"}}"#);
    shared.ipc(&sealed(&shared, &wrong), None);
    assert!(
        call.receiver.try_recv().is_err(),
        "a page that guessed the id answered"
    );
    let own = format!(r#"{{"id":"{id}","word":"{}","ok":"seen"}}"#, call.word);
    shared.ipc(&sealed(&shared, &own), None);
    assert_eq!(call.receiver.try_recv(), Ok(Ok(Value::from("seen"))));
}

#[test]
fn a_reply_from_a_tab_the_call_did_not_go_to_answers_nothing() {
    let shared = events::fresh();
    let call = shared.calls.open();
    shared.calls.aimed(call.id, 2);
    let reply = format!(r#"{{"id":"{}","word":"{}","ok":1}}"#, call.id, call.word);
    shared.ipc(&sealed(&shared, &reply), Some(5));
    assert!(call.receiver.try_recv().is_err());
    shared.ipc(&sealed(&shared, &reply), Some(2));
    assert_eq!(call.receiver.try_recv(), Ok(Ok(Value::from(1))));
}

#[test]
fn a_tab_whose_page_never_came_says_the_address_asked_of_it() {
    let shared = events::fresh();
    let at = shared.navigation_requested(3);
    assert!(shared.unloaded.asked(3, "http://localhost:5175/", at));
    assert!(shared.unloaded.failed(3, at).is_some());
    crate::lock(&shared.tabs).push(Tab {
        id: 3,
        url: String::new(),
        title: String::new(),
    });
    // The line it shows instead is a blank document's: the tab keeps the address.
    shared.page_load(3, &PageLoadEvent::Started, "about:blank");
    let said = |shared: &Shared| crate::lock(&shared.tabs)[0].url.clone();
    assert_eq!(said(&shared), "http://localhost:5175/");
    // Closed, nothing of it is left for the next tab of its number.
    shared.forget(&[Key::Web(3)]);
    shared.page_load(3, &PageLoadEvent::Started, "about:blank");
    assert_eq!(said(&shared), "about:blank");
}

#[test]
fn a_page_with_no_title_of_its_own_is_named_for_the_file_it_shows() {
    let named = |url: &str, title: &str| {
        let (id, url, title) = (4, url.to_owned(), title.to_owned());
        Tab { id, url, title }.name()
    };
    // A film, a sound, a drawn picture: the document WebKit makes for one has no title, and
    // it never says such a document has finished loading, so the name waits for nothing.
    let film = "http://127.0.0.1:5175/k3/first%20cut.mp4";
    assert_eq!(named(film, ""), "first cut.mp4");
    assert_eq!(named(film, "  "), "first cut.mp4");
    // A title the page gives is the tab's.
    assert_eq!(named("http://a.test/shop/cart.html", "Shop"), "Shop");
    // An address that names no file is the name, as it was: a route says less than its place.
    for place in [
        "http://a.test/shop/",
        "http://localhost:5173/about",
        "about:blank",
    ] {
        assert_eq!(named(place, ""), place);
    }
    assert_eq!(named("", ""), "");
}
