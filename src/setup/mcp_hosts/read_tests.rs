//! Tests of `read`: a configuration with comments in it.

use super::{Kept, uncommented};

const AT: &[&str] = &["mcpServers", "llx-desk", "command"];

#[test]
fn json_with_comments_is_read_as_the_agent_reads_it() {
    let text = r#"{
  // The servers. A "quoted" word, and an address: https://example.com
  "mcpServers": {
    /* ours,
       over two lines */
    "llx-desk": { "command": "/Apps/LLX Desk.app/x", "args": ["mcp"] }, // last
    "web": { "url": "https://example.com/a//b", "note": "/* kept */ \\" }
  }
}"#;
    assert_eq!(
        Kept::Json(AT).entry(text).as_deref(),
        Some("/Apps/LLX Desk.app/x")
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
