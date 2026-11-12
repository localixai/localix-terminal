//! Tests of the eras: which one a request is of, how each opens, and what a modern result says.

use serde_json::json;

use super::{Era, LEGACY, MODERN, discovered, initialized, of};

/// The `_meta` of a modern request in revision `revision`.
fn meta(revision: &str) -> serde_json::Value {
    json!({"_meta": {
        "io.modelcontextprotocol/protocolVersion": revision,
        "io.modelcontextprotocol/clientCapabilities": {},
    }})
}

#[test]
fn the_revision_asked_is_spoken_when_known_else_the_newest_legacy_one() {
    let known = initialized(&json!({"protocolVersion": "2025-03-26"}));
    assert_eq!(known["protocolVersion"], "2025-03-26");
    let newest = initialized(&json!({"protocolVersion": "2025-11-25"}));
    assert_eq!(newest["protocolVersion"], "2025-11-25");
    // The modern revision has no `initialize`: a host that asks for it there, or for one
    // unknown, is spoken to in the newest revision that has.
    for asked in ["1999-01-01", MODERN] {
        let other = initialized(&json!({"protocolVersion": asked}));
        assert_eq!(other["protocolVersion"], LEGACY[0], "{asked}");
    }
}

#[test]
fn a_host_that_searches_tools_by_the_instructions_finds_every_kind_of_tab() {
    for said in [initialized(&json!({})), discovered()] {
        let said = said["instructions"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase();
        for kind in ["page", "terminal", "board", "card", "screenshot"] {
            assert!(said.contains(kind), "the instructions name no {kind}");
        }
    }
}

#[test]
fn a_request_that_names_its_revision_is_modern_and_any_other_legacy() {
    assert_eq!(of("tools/list", &json!({})), Ok(Era::Legacy));
    assert_eq!(of("tools/list", &json!(null)), Ok(Era::Legacy));
    assert_eq!(
        of("initialize", &json!({"protocolVersion": MODERN})),
        Ok(Era::Legacy)
    );
    assert_eq!(of("tools/call", &meta(MODERN)), Ok(Era::Modern));
    assert_eq!(of("server/discover", &meta(MODERN)), Ok(Era::Modern));
}

#[test]
fn a_modern_request_is_refused_as_the_protocol_says_when_it_cannot_be_served() {
    // A revision not served: the ones that are, so the host tries again with one of them.
    let unknown = of("tools/list", &meta("1900-01-01")).unwrap_err();
    assert_eq!(unknown.code, -32022);
    let data = unknown.data.unwrap();
    assert_eq!(data["requested"], "1900-01-01");
    assert_eq!(data["supported"][0], MODERN);
    assert!(
        data["supported"]
            .as_array()
            .unwrap()
            .contains(&json!("2025-11-25"))
    );
    // A legacy revision is not served request by request either.
    assert_eq!(
        of("tools/list", &meta("2025-11-25")).unwrap_err().code,
        -32022
    );
    // Malformed: no capabilities beside the revision, and a discovery that names no revision.
    let bare = json!({"_meta": {"io.modelcontextprotocol/protocolVersion": MODERN}});
    assert_eq!(of("tools/list", &bare).unwrap_err().code, -32602);
    assert_eq!(of("server/discover", &json!({})).unwrap_err().code, -32602);
}

#[test]
fn discovery_says_every_revision_served_and_what_the_server_is() {
    let said = discovered();
    assert_eq!(said["resultType"], "complete");
    let versions: Vec<_> = (said["supportedVersions"].as_array().unwrap().iter())
        .map(|version| version.as_str().unwrap())
        .collect();
    assert_eq!(versions[0], MODERN);
    assert_eq!(versions[1..], LEGACY);
    assert!(said["capabilities"]["tools"].is_object());
    let info = &said["_meta"]["io.modelcontextprotocol/serverInfo"];
    assert_eq!(info["name"], "llxterm");
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert!(said["ttlMs"].as_u64().is_some() && said["cacheScope"] == "public");
}

#[test]
fn only_a_modern_result_says_its_type_and_how_long_a_list_may_be_kept() {
    let legacy = Era::Legacy.listed(json!({"tools": []}));
    assert_eq!(legacy, json!({"tools": []}));
    let done = Era::Modern.complete(json!({"content": [], "isError": true}));
    assert_eq!(done["resultType"], "complete");
    assert!(done["isError"] == true && done.get("ttlMs").is_none());
    let listed = Era::Modern.listed(json!({"tools": []}));
    assert_eq!(listed["resultType"], "complete");
    assert!(listed["ttlMs"].as_u64().is_some() && listed["cacheScope"] == "public");
}
