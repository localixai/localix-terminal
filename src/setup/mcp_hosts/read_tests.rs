//! Tests of `read`: a configuration with comments in it, and a TOML table's values.

use super::{Kept, uncommented};

const AT: &[&str] = &["mcpServers", "llxterm", "command"];

#[test]
fn json_with_comments_is_read_as_the_agent_reads_it() {
    let text = r#"{
  // The servers. A "quoted" word, and an address: https://example.com
  "mcpServers": {
    /* ours,
       over two lines */
    "llxterm": { "command": "/Apps/Localix Terminal.app/x", "args": ["mcp"] }, // last
    "web": { "url": "https://example.com/a//b", "note": "/* kept */ \\" }
  }
}"#;
    assert_eq!(
        Kept::Json(AT).entry(text).as_deref(),
        Some("/Apps/Localix Terminal.app/x")
    );
    // What looks like a comment inside a string is the string's own.
    let plain = uncommented(text);
    assert!(plain.contains("https://example.com/a//b") && plain.contains("/* kept */"));
    assert!(!plain.contains("The servers") && !plain.contains("ours") && !plain.contains("last"));
    assert_eq!(plain.lines().count(), text.lines().count() - 1);
    // Text that is no JSON either way has no entry.
    assert_eq!(Kept::Json(AT).entry("// only a comment"), None);
    assert_eq!(Kept::Json(AT).entry("{ nope"), None);
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
    let value = |table, key| super::toml_value(text, table, key);
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
