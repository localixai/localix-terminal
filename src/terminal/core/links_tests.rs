//! Tests of `links`: a cell links to the address its program gave it, else to an http or https
//! address written in its line, found whole across the rows it wraps over and without the
//! punctuation that ends a sentence; every stretch of it in view is told.

use super::super::Core;
use super::addresses;
use crate::surface::{Link, Span};

fn core(rows: u16, columns: u16) -> Core {
    match Core::new(rows, columns) {
        Ok(core) => core,
        Err(why) => panic!("a core of {rows}×{columns}: {why}"),
    }
}

fn found(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    (addresses(&chars).into_iter())
        .map(|(from, to)| chars.get(from..to).unwrap_or_default().iter().collect())
        .collect()
}

#[test]
fn addresses_end_before_what_ends_a_sentence() {
    assert_eq!(
        found("(see https://a.test/b?c=1)."),
        ["https://a.test/b?c=1"]
    );
    assert_eq!(
        found("http://x.test, and \"https://y.test/z\""),
        ["http://x.test", "https://y.test/z"]
    );
    assert_eq!(
        found("ftp://x.test mailto:a@b.test https:// http://a"),
        [""; 0]
    );
}

#[test]
fn a_programs_link_covers_its_cells() {
    let mut core = core(5, 40);
    core.feed(b"\x1b]8;;https://x.test/a\x07link\x1b]8;;\x07 plain");
    let link = Link {
        uri: "https://x.test/a".into(),
        spans: vec![Span {
            y: 0,
            from: 0,
            to: 3,
        }],
    };
    assert_eq!(core.link_at(2, 0), Some(link));
    assert_eq!(core.link_at(6, 0), None);
}

#[test]
fn an_address_is_found_whole_across_the_rows_it_wraps_over() {
    let mut core = core(5, 20);
    core.feed(b"see https://example.test/a/long/path. ok");
    let want = Link {
        uri: "https://example.test/a/long/path".into(),
        spans: vec![
            Span {
                y: 0,
                from: 4,
                to: 19,
            },
            Span {
                y: 1,
                from: 0,
                to: 15,
            },
        ],
    };
    assert_eq!(core.link_at(3, 1).as_ref(), Some(&want));
    assert_eq!(core.link_at(5, 0), Some(want));
    assert_eq!(core.link_at(16, 1), None, "the full stop after it");
    assert_eq!(core.link_at(1, 0), None);
}

#[test]
fn an_address_after_wide_characters_is_placed_by_its_cells() {
    let mut core = core(5, 40);
    core.feed("日本 https://a.test/x".as_bytes());
    let link = core.link_at(6, 0).map(|link| link.spans);
    assert_eq!(
        link,
        Some(vec![Span {
            y: 0,
            from: 5,
            to: 20,
        }])
    );
}
