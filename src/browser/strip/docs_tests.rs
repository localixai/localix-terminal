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

#[test]
fn a_tab_that_shows_a_file_carries_that_files_picture() {
    let listed = |items: &[Item]| -> serde_json::Value {
        let drawn = script(
            items,
            Key::Term(2),
            Some(true),
            &Projects {
                shown: "proj",
                places: vec![],
            },
        );
        serde_json::from_str(drawn.strip_prefix("S(").unwrap().strip_suffix(')').unwrap()).unwrap()
    };
    // A file in the editor: the picture the tree gives its name, by the path it shows.
    let mut file = item(Key::parse("f1").unwrap(), "main.rs", "proj");
    file.detail = "src/main.rs".into();
    // A page named for the file its address shows (a film, a picture): that file's picture.
    let mut film = item(Key::Web(3), "first cut.mp4", "proj");
    film.detail = "http://127.0.0.1:4000/desk-1/first%20cut.mp4".into();
    // A page with a title of its own is a page, whatever its address ends in.
    let mut site = item(Key::Web(4), "Shop", "proj");
    site.detail = "https://shop.test/index.html".into();
    let data = listed(&[file, film, site, item(Key::Term(2), "zsh", "proj")]);
    assert_eq!(data["tabs"][0]["picture"], file_icons::of("main.rs"));
    assert_eq!(data["tabs"][1]["picture"], file_icons::of("first cut.mp4"));
    assert!(data["tabs"][2].get("picture").is_none());
    assert!(data["tabs"][3].get("picture").is_none());
    assert_ne!(file_icons::of("main.rs"), file_icons::of("first cut.mp4"));
}
