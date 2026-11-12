//! Tests of `codex`: the words `codex mcp add` is run with, the table of headers written after,
//! and what its `config.toml` says it has, read as Codex writes it.

use super::super::server::Wanted;
use super::super::tests::{command, env_at, plugin, remote, server, target};
use super::{add_words, headers_table, seen};
use crate::catalog::{Env, PluginFormat};
use crate::testing::Scratch;

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// As Codex 0.166 wrote them, with a plugin it was given.
const CONFIG: &str = r#"model = "o4"

[mcp_servers.weather]
command = "npx"
args = ["-y", "@acme/weather@1.0.0"]

[mcp_servers.weather.env]
API_KEY = "k"

[mcp_servers.w]
url = "https://w.example/mcp"

[marketplaces.acme]
last_updated = "2026-11-01T10:00:00Z"
source_type = "git"
source = "https://github.com/acme/plugins"

[plugins."pdf@acme"]
enabled = true

[plugins."off@acme"]
enabled = false
"#;

#[test]
fn a_program_is_added_with_its_environment_after_its_key() {
    let runs = command("npx", &["-y", "@acme/weather"]);
    let wanted = Wanted {
        key: "weather".into(),
        runs: &runs,
        env: pairs(&[("API_KEY", "k")]),
        headers: vec![],
        secrets: vec![],
    };
    let words = [
        "mcp",
        "add",
        "weather",
        "--env",
        "API_KEY=k",
        "--",
        "npx",
        "-y",
        "@acme/weather",
    ];
    assert_eq!(add_words(&wanted), words);
}

#[test]
fn an_addresss_headers_are_a_table_written_after_its_entry() {
    let runs = remote("https://w.example/mcp", false);
    let wanted = Wanted {
        key: "w".into(),
        runs: &runs,
        env: vec![],
        headers: pairs(&[("Authorization", "Bearer \"t\"")]),
        secrets: vec![],
    };
    assert_eq!(
        add_words(&wanted),
        ["mcp", "add", "w", "--url", "https://w.example/mcp"]
    );
    let table = "\n[mcp_servers.w.http_headers]\n\"Authorization\" = \"Bearer \\\"t\\\"\"\n";
    assert_eq!(headers_table(&wanted).as_deref(), Some(table));
    let bare = Wanted {
        headers: vec![],
        ..wanted
    };
    assert_eq!(headers_table(&bare), None);
}

#[test]
fn an_entry_is_read_from_the_toml_codex_wrote() {
    let weather = seen(CONFIG, "weather").unwrap();
    assert_eq!(weather.command.as_deref(), Some("npx"));
    assert_eq!(weather.args, ["-y", "@acme/weather@1.0.0"]);
    assert_eq!(
        seen(CONFIG, "w").unwrap().url.as_deref(),
        Some("https://w.example/mcp")
    );
    assert_eq!(seen(CONFIG, "none"), None);
    // A table's name may be quoted.
    let quoted = "[mcp_servers.\"my.server\"]\ncommand = 'uvx'\nargs = []\n";
    assert_eq!(
        seen(quoted, "my.server").unwrap().command.as_deref(),
        Some("uvx")
    );
}

#[test]
fn what_codex_has_is_read_from_its_home_where_its_variable_moves_it() {
    let scratch = Scratch::new();
    let env = Env {
        vars: [(
            "CODEX_HOME".to_string(),
            scratch.path().join("cx").display().to_string(),
        )]
        .into(),
        ..env_at(scratch.path())
    };
    scratch.file("cx/config.toml", CONFIG);
    let older = server(
        "acme/weather",
        command("npx", &["-y", "@acme/weather@1.2.0"]),
        vec![],
    );
    assert_eq!(target("codex").has(&older, &env).as_deref(), Some("1.0.0"));
    let pdf = plugin("pdf", "acme", PluginFormat::Claude);
    assert_eq!(target("codex").has(&pdf, &env).as_deref(), Some(""));
    scratch.file("cx/plugins/cache/acme/pdf/1.2.3/README.md", "");
    assert_eq!(target("codex").has(&pdf, &env).as_deref(), Some("1.2.3"));
    assert_eq!(
        target("codex").has(&plugin("off", "acme", PluginFormat::Codex), &env),
        None
    );
    assert_eq!(target("codex").has(&pdf, &env_at(scratch.path())), None);
}
