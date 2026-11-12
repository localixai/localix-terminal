//! Tests of `catalog::source::registry`: pages as the registry answers them, read through a
//! pass and the passes after it.

use super::*;
use crate::catalog::model::{Runs, Server};
use crate::catalog::source::tests::{Fake, new};

/// 2026-11-12T13:18:20Z.
const NOW: u64 = 1_794_489_500;

const REGISTRY: Registry = Registry { now: || NOW };

/// The registry's first page, two of its servers kept, and the cursor of the next.
const FIRST: &str = r#"{"servers":[
{"server":{"$schema":"https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json","name":"ac.inference.sh/mcp","description":"run any ai model. compose agents, stack knowledge, connect tools. one api, pay per run.","title":"inference.sh","version":"2.0.1","remotes":[{"type":"streamable-http","url":"https://api.inference.sh/mcp"}]},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active","publishedAt":"2026-07-27T10:44:51.359634Z","updatedAt":"2026-07-27T10:44:51.359634Z","isLatest":true}}},
{"server":{"$schema":"https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json","name":"ac.snag/snag","description":"Gives your coding agent the captured console, network, replay and screenshot evidence for a bug.","version":"1.0.0","websiteUrl":"https://snag.ac","remotes":[{"type":"streamable-http","url":"https://mcp.snag.ac/mcp"}]},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active","updatedAt":"2026-09-09T10:35:53.194664Z","isLatest":true}}}
],"metadata":{"nextCursor":"ac.snag/snag:1.0.0","count":2}}"#;

/// The last page: a server that is a package, one with no way to reach it, and none after.
const LAST: &str = r#"{"servers":[
{"server":{"name":"io.github.someone/files-server","description":"Files","version":"0.3.0","repository":{"url":"https://github.com/someone/files-server","source":"github"},"packages":[{"registryType":"npm","identifier":"files-server","version":"0.3.0","transport":{"type":"stdio"}}]},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active","isLatest":true}}},
{"server":{"name":"com.example/nuget-only","description":"x","version":"1.0.0","packages":[{"registryType":"nuget","identifier":"X","transport":{"type":"stdio"}}]},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active","isLatest":true}}}
],"metadata":{"count":2}}"#;

/// What changed since: one deleted, one updated.
const CHANGED: &str = r#"{"servers":[
{"server":{"name":"ac.inference.sh/mcp","description":"gone","version":"2.0.2"},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"deleted","isLatest":true}}},
{"server":{"name":"ac.snag/snag","description":"Bug evidence.","version":"1.1.0","remotes":[{"type":"sse","url":"https://mcp.snag.ac/sse"}]},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active","isLatest":true}}}
],"metadata":{"count":2}}"#;

#[test]
fn a_page_lists_each_server_it_can_reach_as_its_registry_writes_it() {
    let Page {
        listings,
        gone,
        next,
    } = read_page("mcp-registry", FIRST).unwrap();
    assert_eq!(next.as_deref(), Some("ac.snag/snag:1.0.0"));
    assert!(gone.is_empty());
    let inference = &listings[0];
    assert_eq!(inference.id, "mcp-registry/ac.inference.sh/mcp");
    assert_eq!(inference.what, What::Server);
    assert_eq!(inference.name, "ac.inference.sh/mcp");
    assert_eq!(inference.title, "inference.sh");
    assert_eq!(inference.author, "ac.inference.sh");
    assert_eq!(inference.version, "2.0.1");
    assert_eq!(
        inference.recipe,
        Recipe::Server(Server {
            runs: Runs::Remote {
                url: "https://api.inference.sh/mcp".into(),
                sse: false,
            },
            needs: vec![],
        })
    );
    // No title: the last part of its name, made readable.
    assert_eq!(listings[1].title, "Snag");
    assert_eq!(listings[1].homepage, "https://snag.ac");
    let Page {
        listings,
        gone,
        next,
    } = read_page("mcp-registry", LAST).unwrap();
    assert_eq!(next, None);
    assert_eq!(listings.len(), 1);
    assert_eq!(
        listings[0].homepage,
        "https://github.com/someone/files-server"
    );
    // One with no way to reach it is not listed, and no longer listed if it was.
    assert_eq!(gone, ["mcp-registry/com.example/nuget-only"]);
}

#[test]
fn the_first_pass_pages_through_and_the_next_asks_only_what_changed_since_it_began() {
    let net = Fake::answering([new(FIRST, None), new(LAST, None), new(CHANGED, None)]);
    let cache = std::env::temp_dir();
    let first = REGISTRY.read(&net, None, &cache).unwrap().unwrap();
    assert!(first.more && !first.whole);
    let last = REGISTRY
        .read(&net, Some(&first.mark), &cache)
        .unwrap()
        .unwrap();
    assert!(!last.more);
    let changed = REGISTRY
        .read(&net, Some(&last.mark), &cache)
        .unwrap()
        .unwrap();
    let asked: Vec<String> = net.asked().into_iter().map(|(url, _)| url).collect();
    assert_eq!(
        asked,
        [
            SERVERS.to_string(),
            format!("{SERVERS}&cursor=ac%2Esnag%2Fsnag%3A1%2E0%2E0"),
            format!("{SERVERS}&include_deleted=true&updated_since=2026%2D11%2D12T13%3A08%3A20Z"),
        ]
    );
    assert_eq!(changed.gone, ["mcp-registry/ac.inference.sh/mcp"]);
    assert_eq!(changed.listings.len(), 1);
    assert_eq!(changed.listings[0].version, "1.1.0");
    assert!(!changed.more);
}

#[test]
fn a_pass_cut_short_by_a_failure_goes_on_from_the_page_it_was_at() {
    let net = Fake::answering([new(FIRST, None), Err("timed out".into()), new(LAST, None)]);
    let cache = std::env::temp_dir();
    let first = REGISTRY.read(&net, None, &cache).unwrap().unwrap();
    assert!(REGISTRY.read(&net, Some(&first.mark), &cache).is_err());
    let resumed = REGISTRY
        .read(&net, Some(&first.mark), &cache)
        .unwrap()
        .unwrap();
    assert!(!resumed.more);
    let asked = net.asked();
    assert_eq!(asked[1], asked[2]);
}

#[test]
fn an_empty_page_ends_a_pass_and_an_answer_that_is_no_page_says_why() {
    let net = Fake::answering([new(r#"{"servers":[],"metadata":{"nextCursor":"x"}}"#, None)]);
    let batch = REGISTRY
        .read(&net, None, &std::env::temp_dir())
        .unwrap()
        .unwrap();
    assert!(!batch.more);
    assert!(read_page("mcp-registry", r#"{"error":"busy"}"#).is_err());
}

#[test]
fn an_older_version_on_a_page_is_passed_over_and_one_id_is_kept_per_server() {
    // As a page asked without `version=latest` answers: the same server at two versions.
    let page = r#"{"servers":[
{"server":{"name":"ac.snag/snag","description":"Old","version":"1.0.0","remotes":[{"type":"streamable-http","url":"https://mcp.snag.ac/mcp"}]},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active","isLatest":false}}},
{"server":{"name":"ac.snag/snag","description":"New","version":"1.1.0","remotes":[{"type":"streamable-http","url":"https://mcp.snag.ac/mcp"}]},
 "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active","isLatest":true}}}
],"metadata":{"count":2}}"#;
    let Page { listings, gone, .. } = read_page("mcp-registry", page).unwrap();
    assert!(gone.is_empty());
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].id, "mcp-registry/ac.snag/snag");
    assert_eq!(listings[0].version, "1.1.0");
    assert_eq!(listings[0].origin, "");
    // Every pass asks for the latest version alone.
    assert!(SERVERS.contains("version=latest"));
}
