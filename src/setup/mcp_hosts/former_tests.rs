//! Tests of `former`: each agent's `llx-desk` server read as 0.1.6 had it added, and taken out
//! only when what it runs is gone or is this app.

use std::path::Path;

use super::super::{HOSTS, host, opencode_server};
use super::gone_or_this;
use crate::testing::Scratch;

/// 0.1.6's program, in its bundle: gone once the update replaced it.
const OLD: &str = "/Users/me/Applications/LLX Desk.app/Contents/MacOS/llx-desk";

#[test]
fn every_agents_llx_desk_server_is_read_as_its_own_command_wrote_it() {
    let exe = Path::new("/Users/me/Applications/Localix Terminal.app/Contents/MacOS/llxterm");
    let stale = |id: &str, text: &str| host(id).unwrap().stale(text, exe, "llx-desk");
    // As this Mac's Codex has it.
    let codex = format!(
        "[mcp_servers.llxterm]\ncommand = \"{}\"\n\n[mcp_servers.llx-desk]\ncommand = \"{OLD}\"\nargs = [\"mcp\"]\n",
        exe.display()
    );
    assert_eq!(stale("codex", &codex).as_deref(), Some(OLD));
    let json =
        format!(r#"{{"mcpServers": {{"llx-desk": {{"command": "{OLD}", "args": ["mcp"]}}}}}}"#);
    assert_eq!(stale("gemini", &json).as_deref(), Some(OLD));
    assert_eq!(stale("copilot", &json).as_deref(), Some(OLD));
    let opencode =
        format!(r#"{{"mcp": {{"llx-desk": {{"type": "local", "command": ["{OLD}", "mcp"]}}}}}}"#);
    assert_eq!(stale("opencode", &opencode).as_deref(), Some(OLD));
    let yaml = format!("mcp_servers:\n  llx-desk:\n    command: {OLD}\n    args:\n      - mcp\n");
    assert_eq!(stale("hermes", &yaml).as_deref(), Some(OLD));
    // This app's own server is never taken for the old one, nor the old for this app's.
    let only_new = format!("[mcp_servers.llxterm]\ncommand = \"{OLD}\"\n");
    assert_eq!(stale("codex", &only_new), None);
    assert_eq!(
        host("codex").unwrap().entry(&codex).as_deref(),
        Some(&*exe.to_string_lossy())
    );
    for one in &HOSTS {
        assert_eq!(
            one.stale("model = 1\n", exe, "llx-desk"),
            None,
            "{}",
            one.id
        );
    }
}

#[test]
fn a_server_is_stale_when_its_program_is_gone_or_is_this_app() {
    let scratch = Scratch::new();
    let exe = std::fs::canonicalize(scratch.file("app/llxterm", "")).unwrap();
    let kept = scratch.file("cargo/bin/llx-desk", "");
    assert!(gone_or_this(OLD, &exe));
    assert!(gone_or_this(&exe.display().to_string(), &exe));
    // An LLX Desk still there is the person's.
    assert!(!gone_or_this(&kept.display().to_string(), &exe));
    // A link to a program that is gone runs nothing.
    let dangling = scratch.path().join("dangling");
    std::os::unix::fs::symlink(scratch.path().join("nothing"), &dangling).unwrap();
    assert!(gone_or_this(&dangling.display().to_string(), &exe));
    // Codex, Gemini CLI, Copilot and Hermes are told the old name to take out; OpenCode's JSON
    // loses that server alone.
    for one in HOSTS.iter().filter(|one| one.remove.is_some()) {
        assert!(
            one.remove.unwrap().contains(&super::super::NAME),
            "{}",
            one.id
        );
    }
    let both = format!(
        r#"{{"mcp": {{"llxterm": {{"command": ["{}"]}}, "llx-desk": {{"command": ["{OLD}"]}}}}}}"#,
        exe.display()
    );
    let written = opencode_server(&both, "llx-desk", None).unwrap();
    assert!(written.contains("\"llxterm\"") && !written.contains("llx-desk"));
}
