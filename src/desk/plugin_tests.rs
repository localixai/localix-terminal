//! What the Claude Code plugin reads from the desk, pinned on both sides: the plugin parses
//! `status` and a `list` as text and keeps a card's columns in its own types, so a change of
//! wording or of a column here fails these tests rather than the pane in a session.

use super::*;

/// A file of the plugin, from the repository's root.
fn plugin(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins/claude-code");
    std::fs::read_to_string(root.join(path)).unwrap()
}

#[test]
fn status_and_a_list_read_as_the_plugin_parses_them() {
    let hooks = plugin("hooks/register.tsx");
    // The plugin takes `window: ` and `terminals: ` lines of `status`, and a tab's line by its key.
    for parsed in ["field('window')", "field('terminals')", r"/^[wtb]\d+ /"] {
        assert!(
            hooks.contains(parsed),
            "the plugin no longer parses {parsed}"
        );
    }

    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&me, &open).unwrap();
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();

    let status = desk.status();
    let lines: Vec<&str> = status.lines().collect();
    assert!(
        lines.iter().any(|line| line.starts_with("window: ")),
        "{status}"
    );
    assert!(lines.contains(&"terminals: 1"), "{status}");

    let listed = desk.window(&me, &json!({"action": "list"})).unwrap();
    let tab = |line: &&str| {
        let key = line.split(' ').next().unwrap_or_default();
        let mut chars = key.chars();
        matches!(chars.next(), Some('w' | 't' | 'b'))
            && !chars.as_str().is_empty()
            && chars.all(|c| c.is_ascii_digit())
            && line.len() > key.len()
    };
    let tabs: Vec<&str> = listed.lines().filter(tab).collect();
    assert_eq!(tabs.len(), 2, "{listed}");
    for key in ["t1 ", "b1 "] {
        assert!(tabs.iter().any(|line| line.starts_with(key)), "{listed}");
    }
    desk.shutdown();
}

#[test]
fn the_plugin_knows_every_column_a_card_can_be_in() {
    let names: Vec<String> = crate::board::Column::ALL
        .iter()
        .map(|column| format!("'{}'", column.name()))
        .collect();
    let expected = format!("column: {}", names.join(" | "));
    let types = plugin("types/index.d.ts");
    assert!(
        types.contains(&expected),
        "plugins/claude-code/types/index.d.ts: BoardCard should say `{expected}`"
    );
}
