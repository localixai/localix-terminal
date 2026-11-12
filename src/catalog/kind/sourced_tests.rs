//! The Store's sources as a person keeps them: what the page is told of them, a source added
//! and removed, and what was installed kept up to date where they left that on.

use serde_json::json;

use super::super::testing::{install, key_need, post, server, sourced};
use crate::kind::Kind as _;

/// What a test's listing may need, as the person types it.
const KEY: &[(&str, &str)] = &[("API_KEY", "k")];

#[test]
fn the_page_is_told_each_source_with_what_it_lists_and_a_row_says_how_far_it_is_trusted() {
    let (store, _, seen, tab, _) = sourced(3, vec![server("weather", "1", vec![])]);
    post(&store, &tab, &json!({"do": "sources"}));
    let told = seen.last("sources");
    assert_eq!(told["removed"], 0);
    let source = &told["sources"][0];
    assert_eq!(
        (&source["id"], &source["title"], &source["trust"]),
        (&json!("reg"), &json!("Listed"), &json!(3))
    );
    assert_eq!(source["listings"], 1);
    assert!(source["ago"].is_u64(), "it was read as the page loaded");

    post(
        &store,
        &tab,
        &json!({"do": "search", "text": "", "show": "server", "seq": 1}),
    );
    let found = seen.last("found");
    assert_eq!(found["listings"][0]["trust"], 3);
    assert_eq!(found["listings"][0]["from"], "Listed");
    assert_eq!(found["facets"][0][1], 1, "{found}");
}

#[test]
fn a_source_removed_takes_what_it_lists_out_of_the_store() {
    let (store, _, seen, tab, _) = sourced(2, vec![server("weather", "1", vec![])]);
    assert_eq!(seen.last("begun")["counts"]["all"], 1);
    // Asked first, in the window's dialog; with no yes it stays.
    post(&store, &tab, &json!({"do": "source-remove", "id": "reg"}));
    assert_eq!(seen.question().title, "Remove Listed?");
    assert_eq!(seen.question().yes, "Remove");
    assert_eq!(seen.last("begun")["counts"]["all"], 1);
    seen.yes(&[]);
    post(&store, &tab, &json!({"do": "source-remove", "id": "reg"}));
    assert_eq!(seen.last("begun")["counts"]["all"], 0);
    assert_eq!(seen.last("sources")["sources"], json!([]));
    assert_eq!(store.summary(), "0 listings, 0 installed");
}

#[test]
fn what_is_not_a_repository_or_lists_nothing_is_not_added_and_the_person_is_told_why() {
    let (store, _, seen, tab, _) = sourced(2, vec![]);
    // The repository is asked for in the window's dialog; with no yes nothing is said or added.
    post(&store, &tab, &json!({"do": "source-add"}));
    let asked = seen.question();
    assert_eq!(
        (asked.title.as_str(), asked.yes.as_str()),
        ("Add a Source", "Add")
    );
    assert_eq!(
        (asked.fields[0].name.as_str(), asked.fields[0].required),
        ("repo", true)
    );
    assert_eq!(asked.fields[0].hint, "owner/repo");

    seen.yes(&[("repo", "nonsense")]);
    post(&store, &tab, &json!({"do": "source-add"}));
    let said = seen.last("said")["said"].as_str().unwrap().to_string();
    assert!(said.contains("owner/repo"), "{said}");

    seen.yes(&[("repo", "acme/tools")]);
    post(&store, &tab, &json!({"do": "source-add"}));
    let said = seen.last("begun")["said"].as_str().unwrap().to_string();
    assert_eq!(
        said,
        "acme/tools has no plugin marketplace and no skills to read"
    );
    assert_eq!(seen.last("sources")["sources"].as_array().unwrap().len(), 1);
}

#[test]
fn a_newer_version_waits_until_the_person_updates_it() {
    let (store, keeps, seen, tab, lists) = sourced(3, vec![server("weather", "1", vec![])]);
    install(&store, &seen, &tab, "reg/weather", KEY);
    *crate::lock(&lists.listings) = vec![server("weather", "2", vec![])];
    post(&store, &tab, &json!({"do": "refresh"}));
    assert_eq!(seen.last("begun")["counts"]["updates"], 1);
    assert_eq!(crate::lock(&keeps.has).len(), 1);

    post(&store, &tab, &json!({"do": "update-all"}));
    assert_eq!(seen.last("begun")["counts"]["updates"], 0);
    assert_eq!(seen.last("row")["update"], false);
}

#[test]
fn update_all_leaves_what_needs_the_persons_values_for_them() {
    let listings = vec![
        server("weather", "1", vec![]),
        server("maps", "1", vec![key_need()]),
    ];
    let (store, _, seen, tab, lists) = sourced(2, listings);
    install(&store, &seen, &tab, "reg/weather", KEY);
    install(&store, &seen, &tab, "reg/maps", KEY);
    *crate::lock(&lists.listings) = vec![
        server("weather", "2", vec![]),
        server("maps", "2", vec![key_need()]),
    ];
    post(&store, &tab, &json!({"do": "refresh"}));
    assert_eq!(seen.last("begun")["counts"]["updates"], 2);

    post(&store, &tab, &json!({"do": "update-all"}));
    assert_eq!(seen.last("begun")["counts"]["updates"], 1);
    post(
        &store,
        &tab,
        &json!({"do": "search", "text": "", "show": "updates", "seq": 1}),
    );
    assert_eq!(seen.last("found")["listings"][0]["id"], "reg/maps");
}
