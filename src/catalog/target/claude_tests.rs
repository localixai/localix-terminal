//! Tests of `claude`: the entry `claude mcp add-json` is given, and what Claude Code's records
//! say it has.

use serde_json::json;

use super::super::server::Wanted;
use super::super::tests::{command, env_at, plugin, remote, target};
use super::{entry, installed};
use crate::catalog::{Env, PluginFormat};
use crate::testing::Scratch;

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

#[test]
fn a_program_is_given_as_a_stdio_entry_with_its_environment() {
    let runs = command("npx", &["-y", "@acme/weather"]);
    let wanted = Wanted {
        key: "weather".into(),
        runs: &runs,
        env: pairs(&[("API_KEY", "k")]),
        headers: vec![],
        secrets: vec![],
    };
    let want = json!({"type": "stdio", "command": "npx", "args": ["-y", "@acme/weather"], "env": {"API_KEY": "k"}});
    assert_eq!(entry(&wanted), want);
}

#[test]
fn an_address_is_given_as_http_or_sse_with_its_headers() {
    let runs = remote("https://w.example/mcp", false);
    let wanted = Wanted {
        key: "w".into(),
        runs: &runs,
        env: vec![],
        headers: pairs(&[("Authorization", "Bearer t")]),
        secrets: vec![],
    };
    let want = json!({"type": "http", "url": "https://w.example/mcp", "headers": {"Authorization": "Bearer t"}});
    assert_eq!(entry(&wanted), want);
    let runs = remote("https://w.example/sse", true);
    let wanted = Wanted {
        key: "w".into(),
        runs: &runs,
        env: vec![],
        headers: vec![],
        secrets: vec![],
    };
    assert_eq!(
        entry(&wanted),
        json!({"type": "sse", "url": "https://w.example/sse"})
    );
}

#[test]
fn a_plugin_is_had_at_user_scope_with_its_version() {
    let records = json!({"version": 2, "plugins": {
        "pdf@acme": [{"scope": "project", "version": "0.1.0"}, {"scope": "user", "installPath": "/x", "version": "1.2.3"}],
        "docs@acme": [{"scope": "project", "version": "1.0.0"}],
        "bare@acme": [{"scope": "user"}],
    }});
    assert_eq!(installed(&records, "pdf@acme").as_deref(), Some("1.2.3"));
    assert_eq!(installed(&records, "docs@acme"), None);
    assert_eq!(installed(&records, "bare@acme").as_deref(), Some(""));
    assert_eq!(installed(&records, "none@acme"), None);
    assert_eq!(
        installed(&json!({"plugins": "not this shape"}), "pdf@acme"),
        None
    );
}

#[test]
fn what_claude_code_has_is_read_from_its_folder_where_its_variable_moves_it() {
    let scratch = Scratch::new();
    let moved = scratch.path().join("elsewhere");
    let env = Env {
        vars: [("CLAUDE_CONFIG_DIR".to_string(), moved.display().to_string())].into(),
        ..env_at(scratch.path())
    };
    let pdf = plugin("pdf", "acme", PluginFormat::Claude);
    let records =
        r#"{"version": 2, "plugins": {"pdf@acme": [{"scope": "user", "version": "1.2.3"}]}}"#;
    scratch.file(".claude/plugins/installed_plugins.json", records);
    assert_eq!(target("claude").has(&pdf, &env), None);
    scratch.file("elsewhere/plugins/installed_plugins.json", records);
    assert_eq!(target("claude").has(&pdf, &env).as_deref(), Some("1.2.3"));
    // Its servers are in `.claude.json` in that folder too.
    let plan = target("claude")
        .plan(
            &super::super::tests::server("w", command("npx", &["w"]), vec![]),
            &env,
        )
        .unwrap();
    assert!(
        plan.steps[0].contains("~/elsewhere/.claude.json"),
        "{:?}",
        plan.steps
    );
}

#[test]
fn a_server_claude_code_wrote_is_had() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let listing = super::super::tests::server("acme/w", command("npx", &["-y", "w"]), vec![]);
    scratch.file(".claude.json", r#"{"mcpServers": {"w": {"type": "stdio", "command": "npx", "args": ["-y", "w"], "env": {}}}}"#);
    assert_eq!(
        target("claude").has(&listing, &env).as_deref(),
        Some("1.0.0")
    );
}
