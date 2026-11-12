//! Tests of `mcp_hosts`: each agent's configuration read as it writes it, and OpenCode's
//! written.

use std::path::Path;

use super::{HOSTS, host, opencode, write_over};

const EXE: &str = "/Users/me/Applications/Localix Terminal.app/Contents/MacOS/llxterm";

#[test]
fn every_agent_is_read_as_its_own_command_writes_it() {
    let read = |id: &str, text: &str| host(id).unwrap().entry(text);
    // As `codex mcp add`, `gemini mcp add`, `copilot mcp add` and `hermes mcp add` wrote them.
    let codex = format!(
        "model = \"o3\"\n\n[mcp_servers.llxterm]\ncommand = \"{EXE}\"\nargs = [\"mcp\"]\n\n[mcp_servers.other]\ncommand = \"x\"\n"
    );
    assert_eq!(read("codex", &codex).as_deref(), Some(EXE));
    // A path with a quote or a backslash in it is kept as a literal, in single quotes, and a
    // comment may follow either.
    let literal =
        "[mcp_servers.llxterm]\ncommand = 'C:\\a \"b\"\\llxterm' # ours\nargs = [\"mcp\"]\n";
    assert_eq!(
        read("codex", literal).as_deref(),
        Some("C:\\a \"b\"\\llxterm")
    );
    let noted = format!("[mcp_servers.llxterm]\ncommand = \"{EXE}\" # ours\n");
    assert_eq!(read("codex", &noted).as_deref(), Some(EXE));
    let json =
        format!(r#"{{"mcpServers": {{"llxterm": {{"command": "{EXE}", "args": ["mcp"]}}}}}}"#);
    assert_eq!(read("gemini", &json).as_deref(), Some(EXE));
    assert_eq!(read("copilot", &json).as_deref(), Some(EXE));
    // Gemini CLI keeps the comments a person wrote in its settings.
    let noted = format!("// mine\n{json}");
    assert_eq!(read("gemini", &noted).as_deref(), Some(EXE));
    let yaml = format!(
        "mcp_servers:\n  other:\n    command: npx\n  llxterm:\n    command: {EXE}\n    args:\n      - mcp\n    enabled: true\nmodel: x\n"
    );
    assert_eq!(read("hermes", &yaml).as_deref(), Some(EXE));
    // None of them has it: not added.
    for one in &HOSTS {
        assert_eq!(one.entry("model = 1\n"), None, "{}", one.id);
    }
    // Another server's command is never taken for its.
    let other = "mcp_servers:\n  llxterm-old:\n    command: /x\n  other:\n    command: /y\n";
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
    assert!(opencode("", exe, true).unwrap().contains("llxterm"));
}

#[test]
fn taking_the_server_out_of_a_configuration_without_it_changes_nothing() {
    let exe = Path::new(EXE);
    // No configuration, no `mcp`, another server only: all as they were, not even reformatted.
    assert_eq!(opencode("", exe, false).unwrap(), "");
    let bare = r#"{"$schema":"s"}"#;
    assert_eq!(opencode(bare, exe, false).unwrap(), bare);
    let other = r#"{"mcp":{"other":{"type":"local"}}}"#;
    assert_eq!(opencode(other, exe, false).unwrap(), other);
}

#[test]
fn a_configuration_is_written_over_whole_and_through_a_link() {
    let scratch = crate::testing::Scratch::new();
    let real = scratch.path().join("dotfiles/opencode.json");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::write(&real, "old").unwrap();
    let link = scratch.path().join("opencode.json");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    write_over(&link, "new").unwrap();
    // The link is still one, and the file it leads to has the text; nothing else is left.
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
    assert_eq!(
        std::fs::read_dir(real.parent().unwrap()).unwrap().count(),
        1
    );
}

#[test]
fn an_entry_an_older_app_added_without_its_timeout_is_not_whole() {
    let copilot = host("copilot").unwrap();
    let old =
        format!(r#"{{"mcpServers": {{"llxterm": {{"command": "{EXE}", "args": ["mcp"]}}}}}}"#);
    assert_eq!(copilot.entry(&old).as_deref(), Some(EXE));
    assert!(!copilot.whole(&old));
    // Any timeout is whole: the person's own number is theirs to keep.
    let new = format!(
        r#"{{"mcpServers": {{"llxterm": {{"command": "{EXE}", "args": ["mcp"], "timeout": 45000}}}}}}"#
    );
    assert!(copilot.whole(&new));
    // The agents whose calls wait long enough by themselves need nothing beside the command.
    assert!(host("gemini").unwrap().whole(&old));
    // Its own command is told the time a call may take.
    let add = copilot.add.unwrap();
    let at = add.iter().position(|arg| *arg == "--timeout").unwrap();
    assert!(add[at + 1].parse::<u64>().unwrap() >= 60_000);
}
