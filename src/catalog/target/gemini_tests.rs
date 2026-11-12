//! Tests of `gemini`: the words `gemini mcp add` is run with, and the entries it writes, read.

use super::super::server::Wanted;
use super::super::tests::{command, env_at, remote, server, target};
use super::add_words;
use crate::catalog::Env;
use crate::testing::Scratch;

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

#[test]
fn a_programs_arguments_come_after_its_command_and_a_double_dash() {
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
        "--scope",
        "user",
        "--env",
        "API_KEY=k",
        "weather",
        "npx",
        "--",
        "-y",
        "@acme/weather",
    ];
    assert_eq!(add_words(&wanted), words);
    let bare = command("weather-mcp", &[]);
    let wanted = Wanted {
        key: "weather".into(),
        runs: &bare,
        env: vec![],
        headers: vec![],
        secrets: vec![],
    };
    assert_eq!(
        add_words(&wanted),
        ["mcp", "add", "--scope", "user", "weather", "weather-mcp"]
    );
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
        "--scope",
        "user",
        "--transport",
        "http",
        "--header",
        "Authorization: Bearer t",
        "w",
        "https://w.example/mcp",
    ];
    assert_eq!(add_words(&wanted), words);
    let sse = remote("https://w.example/sse", true);
    let wanted = Wanted {
        key: "w".into(),
        runs: &sse,
        env: vec![],
        headers: vec![],
        secrets: vec![],
    };
    assert_eq!(add_words(&wanted)[4..6], ["--transport", "sse"]);
}

#[test]
fn what_gemini_wrote_is_had_from_its_settings_where_its_variable_moves_them() {
    let scratch = Scratch::new();
    let env = Env {
        vars: [(
            "GEMINI_CLI_HOME".to_string(),
            scratch.path().join("g").display().to_string(),
        )]
        .into(),
        ..env_at(scratch.path())
    };
    // As Gemini CLI 0.67 wrote them.
    let settings = r#"{
  "mcpServers": {
    "weather": {"command": "npx", "args": ["-y", "@acme/weather"], "env": {"API_KEY": "k"}},
    "w": {"url": "https://w.example/mcp", "type": "http", "headers": {"Authorization": "Bearer t"}}
  }
}"#;
    scratch.file("g/.gemini/settings.json", settings);
    let local = server(
        "acme/weather",
        command("npx", &["-y", "@acme/weather"]),
        vec![],
    );
    assert_eq!(target("gemini").has(&local, &env).as_deref(), Some("1.0.0"));
    let web = server("w", remote("https://w.example/mcp", false), vec![]);
    assert_eq!(target("gemini").has(&web, &env).as_deref(), Some("1.0.0"));
    assert_eq!(target("gemini").has(&local, &env_at(scratch.path())), None);
}
