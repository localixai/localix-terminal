//! Tests of `config`: where each agent keeps its configuration, and how a file of it is read
//! and written.

use std::path::{Path, PathBuf};

use super::{
    claude_dir, claude_json, config_file, json, opencode_server, shown, toml_value, uncommented,
    write_over,
};

/// A reader of the variables `set`.
fn vars<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    |name| {
        (set.iter())
            .find(|(known, _)| *known == name)
            .map(|(_, value)| (*value).to_string())
    }
}

fn config(id: &str, set: &[(&str, &str)]) -> PathBuf {
    config_file(id, Path::new("/Users/me"), vars(set)).unwrap()
}

#[test]
fn every_agent_keeps_its_configuration_under_the_home_directory_until_it_is_moved() {
    let under = |id: &str| config(id, &[]).display().to_string();
    assert_eq!(under("codex"), "/Users/me/.codex/config.toml");
    assert_eq!(under("gemini"), "/Users/me/.gemini/settings.json");
    assert_eq!(under("copilot"), "/Users/me/.copilot/mcp-config.json");
    assert_eq!(
        under("opencode"),
        "/Users/me/.config/opencode/opencode.json"
    );
    assert_eq!(
        config_file("hermes", Path::new("/Users/me"), |_| None),
        None
    );
    // Each agent's own variable names the folder in place of its usual one.
    let moved = |id: &str, var: &str| config(id, &[(var, "/work/x")]).display().to_string();
    assert_eq!(moved("codex", "CODEX_HOME"), "/work/x/config.toml");
    assert_eq!(
        moved("gemini", "GEMINI_CLI_HOME"),
        "/work/x/.gemini/settings.json"
    );
    assert_eq!(moved("copilot", "COPILOT_HOME"), "/work/x/mcp-config.json");
    assert_eq!(
        moved("opencode", "XDG_CONFIG_HOME"),
        "/work/x/opencode/opencode.json"
    );
    // Another agent's variable, or one set to nothing, moves none.
    assert_eq!(moved("codex", "COPILOT_HOME"), under("codex"));
    assert_eq!(
        config("codex", &[("CODEX_HOME", " ")]),
        config("codex", &[])
    );
}

#[test]
fn claude_code_keeps_its_folder_and_its_servers_file_where_its_variable_says() {
    let home = Path::new("/Users/me");
    assert_eq!(claude_dir(home, |_| None), home.join(".claude"));
    assert_eq!(claude_json(home, |_| None), home.join(".claude.json"));
    let set = [("CLAUDE_CONFIG_DIR", "/work/claude")];
    assert_eq!(claude_dir(home, vars(&set)), Path::new("/work/claude"));
    assert_eq!(
        claude_json(home, vars(&set)),
        Path::new("/work/claude/.claude.json")
    );
}

#[test]
fn a_file_is_shown_from_the_home_directory_when_it_is_under_it() {
    let home = Path::new("/Users/me");
    assert_eq!(
        shown(Path::new("/Users/me/.codex/config.toml"), home),
        "~/.codex/config.toml"
    );
    assert_eq!(
        shown(Path::new("/work/x/config.toml"), home),
        "/work/x/config.toml"
    );
}

#[test]
fn json_with_comments_is_read_as_the_agent_reads_it() {
    let text = r#"{
  // The servers. A "quoted" word, and an address: https://example.com
  "mcpServers": {
    /* ours,
       over two lines */
    "web": { "url": "https://example.com/a//b", "note": "/* kept */ \\" } // last
  }
}"#;
    let read = json(text).unwrap();
    assert_eq!(read["mcpServers"]["web"]["url"], "https://example.com/a//b");
    // What looks like a comment inside a string is the string's own.
    let plain = uncommented(text);
    assert!(plain.contains("https://example.com/a//b") && plain.contains("/* kept */"));
    assert!(!plain.contains("The servers") && !plain.contains("ours") && !plain.contains("last"));
    assert_eq!(plain.lines().count(), text.lines().count() - 1);
    // Text that is no JSON either way is none.
    assert_eq!(json("// only a comment"), None);
    assert_eq!(json("{ nope"), None);
}

#[test]
fn a_toml_tables_value_is_read_as_codex_writes_it() {
    let text = r#"model = "o4"

[mcp_servers.weather]
command = "npx"
args = ["-y", "@acme/weather@1.0.0"]

[mcp_servers.weather.env]
command = "not this table's"

[mcp_servers.literal]
command = 'C:\tools\x.exe'

[plugins."pdf@acme"]
enabled = true
"#;
    let value = |table, key| toml_value(text, table, key);
    assert_eq!(value("mcp_servers.weather", "command"), Some("npx".into()));
    let args = serde_json::json!(["-y", "@acme/weather@1.0.0"]);
    assert_eq!(value("mcp_servers.weather", "args"), Some(args));
    assert_eq!(
        value("mcp_servers.literal", "command"),
        Some(r"C:\tools\x.exe".into())
    );
    assert_eq!(value("plugins.\"pdf@acme\"", "enabled"), Some(true.into()));
    assert_eq!(value("mcp_servers.weather", "url"), None);
    assert_eq!(value("mcp_servers", "command"), None);
    assert_eq!(value("model", "model"), None);
}

#[test]
fn opencode_keeps_the_rest_and_takes_out_only_the_server_named() {
    let had = r#"{"$schema": "s", "mcp": {"other": {"type": "local", "command": ["x"]}}}"#;
    let entry = serde_json::json!({"type": "local", "command": ["weather"], "enabled": true});
    let on = opencode_server(had, "weather", Some(entry)).unwrap();
    let read = json(&on).unwrap();
    assert_eq!(read["mcp"]["weather"]["command"][0], "weather");
    assert!(read["mcp"]["other"].is_object() && read["$schema"] == "s");
    let off = opencode_server(&on, "weather", None).unwrap();
    assert!(json(&off).unwrap()["mcp"].get("weather").is_none());
    // No configuration, no `mcp`, another server only: all as they were, not even reformatted.
    assert_eq!(opencode_server("", "weather", None).unwrap(), "");
    let bare = r#"{"$schema":"s"}"#;
    assert_eq!(opencode_server(bare, "weather", None).unwrap(), bare);
    assert!(opencode_server("[1]", "weather", None).is_err());
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
    // A file in a folder that is not there yet has it made.
    let fresh = scratch.path().join("new/folder/config.toml");
    write_over(&fresh, "x = 1\n").unwrap();
    assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "x = 1\n");
}
