//! Tests of `copilot`: the words `copilot mcp add` is run with, the entries it writes, read, and
//! the plugins its settings have on.

use super::super::server::Wanted;
use super::super::tests::{command, env_at, plugin, remote, server, target};
use super::add_words;
use crate::catalog::{Env, PluginFormat};
use crate::testing::Scratch;

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

#[test]
fn a_program_is_added_after_a_double_dash() {
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
        "--env",
        "API_KEY=k",
        "weather",
        "--",
        "npx",
        "-y",
        "@acme/weather",
    ];
    assert_eq!(add_words(&wanted), words);
}

#[test]
fn an_address_is_added_with_its_transport_and_headers() {
    let runs = remote("https://w.example/mcp", false);
    let wanted = Wanted {
        key: "w".into(),
        runs: &runs,
        env: vec![],
        headers: pairs(&[("Authorization", "Bearer t")]),
        secrets: vec![],
    };
    let words = [
        "mcp",
        "add",
        "--transport",
        "http",
        "--header",
        "Authorization: Bearer t",
        "w",
        "https://w.example/mcp",
    ];
    assert_eq!(add_words(&wanted), words);
}

#[test]
fn what_copilot_wrote_is_had_from_its_home() {
    let scratch = Scratch::new();
    let env = Env {
        vars: [(
            "COPILOT_HOME".to_string(),
            scratch.path().join("cp").display().to_string(),
        )]
        .into(),
        ..env_at(scratch.path())
    };
    // As Copilot 1.0.107 wrote them.
    let config = r#"{"mcpServers": {
  "weather": {"tools": ["*"], "type": "local", "command": "npx", "args": ["-y", "@acme/weather"], "env": {"API_KEY": "k"}},
  "w": {"tools": ["*"], "type": "http", "url": "https://w.example/mcp", "headers": {}}
}}"#;
    scratch.file("cp/mcp-config.json", config);
    let local = server("weather", command("npx", &["-y", "@acme/weather"]), vec![]);
    assert_eq!(
        target("copilot").has(&local, &env).as_deref(),
        Some("1.0.0")
    );
    let web = server("w", remote("https://w.example/mcp", false), vec![]);
    assert_eq!(target("copilot").has(&web, &env).as_deref(), Some("1.0.0"));
    scratch.file(
        "cp/settings.json",
        r#"{"enabledPlugins": {"pdf@acme": true, "off@acme": false}}"#,
    );
    let on = plugin("pdf", "acme", PluginFormat::Claude);
    assert_eq!(target("copilot").has(&on, &env).as_deref(), Some(""));
    assert_eq!(
        target("copilot").has(&plugin("off", "acme", PluginFormat::Claude), &env),
        None
    );
    assert_eq!(
        target("copilot").has(&plugin("pdf", "acme", PluginFormat::Codex), &env),
        None
    );
}
