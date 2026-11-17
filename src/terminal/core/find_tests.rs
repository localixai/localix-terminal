//! Find over the history and the screen: case folded in every script, counted from the top and
//! started at the newest, stepping up with Next and down with Previous around the ends, found
//! across a wrapped line, new output looked in again, the match shown scrolled into view.

use ghostty_vt_sys as vt;

use crate::surface::{Count, FindStep, Span};
use crate::terminal::Core;

fn needle(core: &mut Core, text: &str) -> Count {
    core.find(&FindStep::Needle(text.to_string()))
}

#[test]
fn case_is_folded_in_every_script() {
    let mut core = Core::new(4, 30).unwrap();
    core.feed("Привет мир\r\nSTRASSE Straße\r\nNo match".as_bytes());
    assert_eq!(needle(&mut core, "привет").total, 1);
    assert_eq!(needle(&mut core, "strasse").total, 2, "ß folds to ss");
    assert_eq!(needle(&mut core, "MATCH").total, 1);
    assert_eq!(needle(&mut core, "absent"), Count { at: 0, total: 0 });
}

#[test]
fn matches_count_from_the_top_and_next_goes_up_from_the_newest() {
    let mut core = Core::new(4, 30).unwrap();
    core.feed(b"one x\r\ntwo x\r\nthree x");
    assert_eq!(needle(&mut core, "x"), Count { at: 3, total: 3 });
    assert_eq!(core.find(&FindStep::Next), Count { at: 2, total: 3 });
    assert_eq!(core.find(&FindStep::Next), Count { at: 1, total: 3 });
    assert_eq!(
        core.find(&FindStep::Next),
        Count { at: 3, total: 3 },
        "round past the top"
    );
    assert_eq!(
        core.find(&FindStep::Previous),
        Count { at: 1, total: 3 },
        "and past the bottom"
    );
    assert_eq!(core.find(&FindStep::Close), Count::default());
}

#[test]
fn the_matches_in_view_and_the_one_shown() {
    let mut core = Core::new(4, 30).unwrap();
    core.feed(b"find me\r\nand me");
    needle(&mut core, "me");
    let (spans, shown) = core.found_in_view(0, 4);
    assert_eq!(
        spans,
        [
            Span {
                y: 0,
                from: 5,
                to: 6
            },
            Span {
                y: 1,
                from: 4,
                to: 5
            }
        ]
    );
    assert_eq!(shown, Some(spans[1]));
}

#[test]
fn a_match_across_a_wrapped_line_and_wide_characters() {
    let mut core = Core::new(4, 6).unwrap();
    core.feed("abcdneedle 日本語\r\n".as_bytes());
    assert_eq!(needle(&mut core, "needle").total, 1);
    let (spans, _) = core.found_in_view(0, 4);
    assert_eq!(
        spans,
        [
            Span {
                y: 0,
                from: 4,
                to: 5
            },
            Span {
                y: 1,
                from: 0,
                to: 3
            }
        ]
    );
    needle(&mut core, "本");
    let (spans, _) = core.found_in_view(0, 4);
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans[0].to - spans[0].from,
        1,
        "a wide character's two cells"
    );
}

#[test]
fn new_output_is_looked_in_and_history_is_found_and_shown() {
    let mut core = Core::new(4, 30).unwrap();
    core.feed(b"needle at the top\r\n");
    for n in 0..50 {
        core.feed(format!("line {n}\r\n").as_bytes());
    }
    assert_eq!(needle(&mut core, "needle"), Count { at: 1, total: 1 });
    let (spans, shown) = core.found_in_view(0, 4);
    assert_eq!(
        shown,
        Some(Span {
            y: 0,
            from: 0,
            to: 5
        }),
        "{spans:?}"
    );
    core.feed(b"another needle\r\n");
    let (_, _) = core.found_in_view(0, 4);
    assert_eq!(core.find(&FindStep::Previous).total, 2);
}

#[test]
fn counted_again_as_output_grows_the_match_shown_stays_and_nothing_scrolls() {
    let mut core = Core::new(4, 30).unwrap();
    core.feed(b"x one\r\nx two\r\n");
    assert_eq!(needle(&mut core, "x"), Count { at: 2, total: 2 });
    assert_eq!(core.find(&FindStep::Next), Count { at: 1, total: 2 });
    for n in 0..20 {
        core.feed(format!("x {n}\r\n").as_bytes());
    }
    let offset = |core: &Core| {
        core.get::<vt::TerminalScrollbar>(vt::TerminalData::SCROLLBAR)
            .offset
    };
    let bar_before = offset(&core);
    assert_eq!(
        core.find(&FindStep::Again),
        Count { at: 1, total: 22 },
        "the new matches are counted, the one shown is still the first"
    );
    assert_eq!(offset(&core), bar_before, "a recount scrolled the view");
    assert_eq!(core.find(&FindStep::Again), Count { at: 1, total: 22 });
}
