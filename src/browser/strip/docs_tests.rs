//! Tests of what the strip's pages are told of a session's documents: each document's session, and
//! the row of the documents in sight.

use super::*;

#[test]
fn a_sessions_document_names_its_session_and_the_row_lists_the_documents_in_sight() {
    let mut doc = item(Key::parse("f1").unwrap(), "main.rs", "proj");
    doc.of = Some("t2".into());
    let drawn = script(
        &[doc, item(Key::Term(2), "zsh", "proj")],
        Key::Term(2),
        Some(true),
        &Projects {
            shown: "proj",
            places: vec![],
        },
    );
    let data: serde_json::Value =
        serde_json::from_str(drawn.strip_prefix("S(").unwrap().strip_suffix(')').unwrap()).unwrap();
    assert_eq!(data["tabs"][0]["of"], "t2");
    assert!(data["tabs"][1].get("of").is_none());

    let keys = [Key::parse("f1").unwrap(), Key::Web(3)];
    assert_eq!(
        docs(Some((Key::Web(3), &keys))),
        r#"DR({"keys":["f1","w3"],"on":"w3"})"#
    );
    assert_eq!(docs(None), "DR(null)");
}
