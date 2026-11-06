//! Tests of `choice`.

use super::*;

#[test]
fn a_pages_menu_reads_as_items_lines_and_submenus_counted_in_order() {
    let entries: Vec<Entry> = serde_json::from_str(
        r#"[{"label":"Open"},{"line":true},
            {"label":"Move To","items":[{"label":"Todo","marked":true},{"label":"Done","off":true}]},
            {"label":"Paste","paste":true}]"#,
    )
    .unwrap();
    let labels: Vec<&str> = (flat(&entries).into_iter())
        .map(|entry| match entry {
            Entry::Item { label, .. } => label.as_str(),
            Entry::Sub { .. } | Entry::Line { .. } => "",
        })
        .collect();
    assert_eq!(labels, ["Open", "Todo", "Done", "Paste"]);
    assert!(matches!(flat(&entries)[3], Entry::Item { paste: true, .. }));
    assert!(matches!(
        flat(&entries)[1],
        Entry::Item { marked: true, .. }
    ));
}

#[test]
fn labels_make_items_with_one_checked() {
    let made = items(&["Page", "Terminal"], Some(1));
    assert!(matches!(&made[1], Entry::Item { marked: true, .. }));
    assert!(matches!(&made[0], Entry::Item { marked: false, .. }));
}
