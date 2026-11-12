//! The Store as an agent uses it: it searches and reads, and what it asks to install waits for
//! the person.

use std::sync::{Arc, Mutex};

use serde_json::json;

use super::super::testing::{key_need, server, store};
use crate::kind::Kind as _;

#[test]
fn what_an_agent_asks_to_install_waits_for_the_person() {
    let (store, keeps, seen, tab) = store(vec![server("weather", "1", vec![])]);
    // What an agent reads of the Store while the person is being asked.
    let read = Arc::new(Mutex::new(String::new()));
    let (reader, kept) = (Arc::clone(&store), Arc::clone(&read));
    seen.while_asked(move || *crate::lock(&kept) = reader.text());
    let install = json!({"store": {"install": "reg/weather", "agent": "keeps"}});

    // The person does not go on.
    let said = store.call(&install, &tab).unwrap();
    assert!(
        said.report
            .starts_with("Asked the person to install reg/weather in Keeps."),
        "{}",
        said.report
    );
    assert!(said.report.contains("Add weather to keeps.json"));
    let asked = seen.question();
    assert_eq!(asked.title, "An agent asks to install WEATHER in Keeps?");
    assert!(!asked.asked, "what an agent raised leaves the keys alone");
    assert!(
        crate::lock(&read).contains("waits for the person's answer: install reg/weather in keeps"),
        "{}",
        crate::lock(&read)
    );
    assert!(
        crate::lock(&keeps.has).is_empty(),
        "nothing is installed on an agent's word"
    );
    let text = store.text();
    assert!(text.contains("The person declined to install reg/weather in keeps."));
    assert!(!text.contains("waits for the person's answer"), "{text}");

    // They do.
    seen.yes(&[]);
    store.call(&install, &tab).unwrap();
    assert!(crate::lock(&keeps.has).contains_key("reg/weather"));
    assert!(
        store
            .text()
            .contains("reg/weather: Added weather to Keeps.")
    );
}

#[test]
fn an_agent_searches_and_reads_and_is_told_what_it_may_not_ask() {
    let (store, _, _, tab) = store(vec![
        server("weather", "1", vec![key_need()]),
        server("maps", "1", vec![]),
    ]);
    let found = store
        .call(&json!({"store": {"search": "maps"}}), &tab)
        .unwrap();
    assert_eq!(found.report, "1 found, 1 shown");
    assert_eq!(
        found.state.unwrap(),
        "reg/maps — MAPS (MCP server by acme): maps over MCP"
    );
    let read = store
        .call(&json!({"store": {"show": "reg/weather"}}), &tab)
        .unwrap();
    let read = read.state.unwrap();
    assert!(read.contains("needs API_KEY (needed): The key"), "{read}");
    assert!(read.contains("keeps: not installed"), "{read}");
    for (asked, why) in [
        (
            json!({"store": {"show": "reg/none"}}),
            "the Store lists nothing as `reg/none`",
        ),
        (
            json!({"store": {"install": "reg/maps"}}),
            "`agent` is needed: which agent to install it in",
        ),
        (
            json!({"store": {"install": "reg/maps", "agent": "nobody"}}),
            "there is no agent `nobody`",
        ),
        (
            json!({"store": {"search": "x", "what": "song"}}),
            "`what` is plugin, skill or server",
        ),
        (
            json!({"store": {"remove": "reg/maps"}}),
            "`store` takes `search`, `show` or `install`",
        ),
    ] {
        assert_eq!(store.call(&asked, &tab).unwrap_err(), why);
    }
}
