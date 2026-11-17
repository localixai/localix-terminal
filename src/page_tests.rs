//! Tests of what the pages share: words never read as markup, data never ending a script's
//! line, and a kit that defines each shared thing once.

use serde_json::json;

use super::*;

const KIT: &str = include_str!("page/kit.js");

#[test]
fn words_are_spelled_out_and_never_read_as_markup() {
    assert_eq!(escaped("a <b> & \"c\""), "a &lt;b&gt; &amp; &quot;c&quot;");
}

#[test]
fn data_holds_no_line_end_a_script_may_not() {
    let told = json(&json!({ "said": "one\u{2028}two\u{2029}" }));
    assert_eq!(told, "{\"said\":\"one\\u2028two\\u2029\"}");
}

#[test]
fn no_page_defines_again_what_the_kit_gives_it() {
    let pages = [
        ("board", include_str!("board/page.js")),
        ("catalog", include_str!("catalog/page.js")),
    ];
    for given in [
        "const post =",
        "const act =",
        "function el(",
        "function systemMenu(",
        "window.chosen =",
        "let offered",
        "createElementNS",
    ] {
        assert!(KIT.contains(given), "the kit gives {given}");
        for (page, script) in pages {
            assert!(!script.contains(given), "{page} defines {given} again");
        }
    }
}

#[test]
fn a_row_that_scrolls_sideways_is_written_once() {
    let across = include_str!("page/across.js");
    let pages = [
        ("strip", include_str!("browser/strip.html")),
        ("catalog", include_str!("catalog/page.js")),
        ("board", include_str!("board/page.js")),
    ];
    for given in [
        "function across(",
        "function rowEdges(",
        "function rowInView(",
        "dataset.more",
    ] {
        assert!(across.contains(given), "the row gives {given}");
        for (page, script) in pages {
            assert!(!script.contains(given), "{page} defines {given} again");
        }
    }
    // The Store's rows scroll; none puts what does not fit under a menu.
    let store = include_str!("catalog/page.js");
    assert!(store.contains("across(row, "));
    assert!(!store.contains("\"More\""));
}

#[test]
fn every_page_that_finds_takes_the_one_find_row() {
    // The browser's toolbar draws the shared markup, and runs its words and keys before its
    // own steps; it keeps a copy of neither.
    let pages = [(
        "toolbar",
        include_str!("browser/strip.html"),
        "<!--FIND_ROW-->",
    )];
    for (page, markup, place) in pages {
        assert!(markup.contains(place), "{page} takes the find row");
        assert!(
            !markup.contains("id=\"find-q\""),
            "{page} draws a find row of its own"
        );
    }
    assert!(FIND_ROW.contains("id=\"find-q\"") && FIND_ROW.contains("class=\"finds\""));
    let joined = [("toolbar", include_str!("browser/strip.rs"))];
    for (page, module) in joined {
        assert!(
            module.contains("page/find.js\")"),
            "{page} runs the row's script"
        );
    }
    for script in [include_str!("browser/page_find.js")] {
        for given in [
            "function counted(",
            "function findText(",
            "addEventListener(\"keydown\"",
        ] {
            assert!(!script.contains(given), "a page defines {given} again");
        }
    }
}
