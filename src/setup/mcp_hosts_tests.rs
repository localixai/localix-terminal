//! Tests of `mcp_hosts`: each agent's configuration read as it writes it, and OpenCode's
//! written.

use std::path::Path;

use super::{HOSTS, host, opencode};

const EXE: &str = "/Users/me/Applications/LLX Desk.app/Contents/MacOS/llx-desk";

#[test]
fn every_agent_is_read_as_its_own_command_writes_it() {
    let read = |id: &str, text: &str| host(id).unwrap().entry(text);
    // As `codex mcp add`, `gemini mcp add`, `copilot mcp add` and `hermes mcp add` wrote them.
    let codex = format!(
        "model = \"o3\"\n\n[mcp_servers.llx-desk]\ncommand = \"{EXE}\"\nargs = [\"mcp\"]\n\n[mcp_servers.other]\ncommand = \"x\"\n"
    );
    assert_eq!(read("codex", &codex).as_deref(), Some(EXE));
    let json =
        format!(r#"{{"mcpServers": {{"llx-desk": {{"command": "{EXE}", "args": ["mcp"]}}}}}}"#);
    assert_eq!(read("gemini", &json).as_deref(), Some(EXE));
    assert_eq!(read("copilot", &json).as_deref(), Some(EXE));
    let yaml = format!(
        "mcp_servers:\n  other:\n    command: npx\n  llx-desk:\n    command: {EXE}\n    args:\n      - mcp\n    enabled: true\nmodel: x\n"
    );
    assert_eq!(read("hermes", &yaml).as_deref(), Some(EXE));
    // None of them has it: not added.
    for one in &HOSTS {
        assert_eq!(one.entry("model = 1\n"), None, "{}", one.id);
    }
    // Another server's command is never taken for its.
    let other = "mcp_servers:\n  llx-desk-old:\n    command: /x\n  other:\n    command: /y\n";
    assert_eq!(read("hermes", other), None);
}

#[test]
fn opencode_keeps_the_rest_and_takes_out_only_its_own() {
    let exe = Path::new(EXE);
    let had = r#"{"$schema": "s", "mcp": {"other": {"type": "local", "command": ["x"]}}}"#;
    let on = opencode(had, exe, true).unwrap();
    assert_eq!(host("opencode").unwrap().entry(&on).as_deref(), Some(EXE));
    assert!(on.contains("\"other\"") && on.contains("\"$schema\""));
    let off = opencode(&on, exe, false).unwrap();
    assert_eq!(host("opencode").unwrap().entry(&off), None);
    assert!(off.contains("\"other\""));
    assert!(opencode("", exe, true).unwrap().contains("llx-desk"));
}
