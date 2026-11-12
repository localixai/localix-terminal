//! The Store's tab as a person uses it: what its page is told as it loads, searches, plans,
//! installs, updates and removes.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::testing::{install, key_need, post, server, store};
use super::*;
use crate::catalog::Batch;

#[test]
fn a_page_that_loads_is_told_what_there_is_and_a_search_finds_rows() {
    let (store, _, seen, tab) = store(vec![
        server("weather", "1", vec![]),
        server("maps", "1", vec![]),
    ]);
    let begun = seen.last("begun");
    assert_eq!(begun["counts"]["all"], 2);
    assert_eq!(begun["counts"]["server"], 2);
    assert_eq!(begun["counts"]["installed"], 0);
    assert_eq!(begun["said"], "", "a Store at rest says nothing");
    assert_eq!(begun["busy"], false);
    post(
        &store,
        &tab,
        &json!({"do": "search", "text": "weath", "show": "all", "offset": 0, "limit": 60, "seq": 7}),
    );
    let found = seen.last("found");
    assert_eq!(
        (found["seq"].as_u64(), found["total"].as_u64()),
        (Some(7), Some(1))
    );
    assert_eq!(found["listings"][0]["id"], "reg/weather");
    assert_eq!(found["listings"][0]["has"], false);
    assert_eq!(found["listings"][0]["author"], "acme");
    assert_eq!(found["listings"][0]["category"], "Other");
    assert_eq!(
        found["listings"][0]["trust"], 0,
        "its source is not one the Store reads"
    );
    post(
        &store,
        &tab,
        &json!({"do": "search", "text": "", "show": "skill", "seq": 8}),
    );
    assert_eq!(seen.last("found")["total"], 0);
    assert_eq!(store.summary(), "2 listings, 0 installed");
}

#[test]
fn an_install_is_asked_in_the_windows_dialog_done_and_noted_and_a_remove_forgets_it() {
    let (store, keeps, seen, tab) = store(vec![server("weather", "1", vec![key_need()])]);
    post(&store, &tab, &json!({"do": "show", "id": "reg/weather"}));
    let shown = seen.last("shown");
    assert_eq!(
        shown["agents"][0],
        json!({"id": "keeps", "name": "Keeps", "present": true, "takes": true, "has": null, "update": false, "busy": null, "said": null})
    );
    assert_eq!(shown["runs_code"], false);
    post(
        &store,
        &tab,
        &json!({"do": "plan", "id": "reg/weather", "agent": "keeps"}),
    );
    // The person is asked in the window's dialog, with what it does and what it needs; they
    // did not go on, and nothing is installed.
    let asked = seen.question();
    assert_eq!(asked.title, "Install WEATHER in Keeps?");
    assert_eq!(asked.points, ["Add weather to keeps.json"]);
    assert_eq!(asked.yes, "Install");
    assert!(asked.asked && !asked.danger);
    let field = &asked.fields[0];
    assert_eq!(
        (field.name.as_str(), field.required, field.secret),
        ("API_KEY", true, true)
    );
    assert!(field.label.contains("The key"), "{}", field.label);
    assert!(crate::lock(&keeps.has).is_empty());

    // A yes without what the listing needs installs nothing.
    install(&store, &seen, &tab, "reg/weather", &[]);
    assert_eq!(seen.last("said")["said"], "API_KEY is needed");
    assert!(crate::lock(&keeps.has).is_empty());

    // With it, and with a value the listing never asked for left out.
    install(
        &store,
        &seen,
        &tab,
        "reg/weather",
        &[("API_KEY", "s3cret"), ("OTHER", "x")],
    );
    let given = crate::lock(&keeps.has).get("reg/weather").cloned().unwrap();
    assert_eq!(
        given,
        BTreeMap::from([("API_KEY".to_string(), "s3cret".to_string())])
    );
    let shown = seen.last("shown");
    assert_eq!(shown["agents"][0]["has"], "");
    assert_eq!(shown["agents"][0]["said"], "Added weather to Keeps.");
    assert_eq!(seen.last("begun")["counts"]["installed"], 1);
    post(
        &store,
        &tab,
        &json!({"do": "search", "text": "", "show": "installed", "seq": 1}),
    );
    assert_eq!(seen.last("found")["listings"][0]["has"], true);
    assert!(store.text().contains("installed: reg/weather"));
    assert!(
        !store.text().contains("s3cret"),
        "a secret is never in what an agent reads"
    );

    post(
        &store,
        &tab,
        &json!({"do": "remove", "id": "reg/weather", "agent": "keeps"}),
    );
    assert!(crate::lock(&keeps.has).is_empty());
    assert_eq!(seen.last("shown")["agents"][0]["has"], Value::Null);
    assert_eq!(seen.last("begun")["counts"]["installed"], 0);
}

#[test]
fn a_newer_listing_of_what_was_installed_is_an_update() {
    let (store, _, seen, tab) = store(vec![server("weather", "1", vec![])]);
    install(&store, &seen, &tab, "reg/weather", &[]);
    assert_eq!(seen.last("begun")["counts"]["updates"], 0);
    let newer = Batch {
        listings: vec![server("weather", "2", vec![])],
        whole: true,
        ..Batch::default()
    };
    store.index.put("reg", 1, &newer, 2).unwrap();
    post(
        &store,
        &tab,
        &json!({"do": "search", "text": "", "show": "updates", "seq": 1}),
    );
    assert_eq!(seen.last("found")["listings"][0]["update"], true);
    post(&store, &tab, &json!({"do": "show", "id": "reg/weather"}));
    assert_eq!(seen.last("shown")["agents"][0]["update"], true);
    assert_eq!(seen.last("begun")["counts"]["updates"], 1);
    // Installed again, it is the one listed.
    install(&store, &seen, &tab, "reg/weather", &[]);
    assert_eq!(seen.last("shown")["agents"][0]["update"], false);
    assert_eq!(seen.last("begun")["counts"]["updates"], 0);
}

#[test]
fn the_page_opens_web_addresses_only_and_an_agent_that_is_not_here_takes_nothing() {
    let (store, _, seen, tab) = store(vec![server("weather", "1", vec![])]);
    post(
        &store,
        &tab,
        &json!({"do": "browse", "url": "https://example.com/weather"}),
    );
    post(
        &store,
        &tab,
        &json!({"do": "browse", "url": "file:///etc/passwd"}),
    );
    post(
        &store,
        &tab,
        &json!({"do": "browse", "url": "javascript:alert(1)"}),
    );
    assert_eq!(*crate::lock(&seen.browsed), ["https://example.com/weather"]);
    store.posted("not json", &tab);
    post(
        &store,
        &tab,
        &json!({"do": "plan", "id": "reg/weather", "agent": "nobody"}),
    );
    assert_eq!(seen.last("said")["said"], "there is no agent `nobody`");
}
