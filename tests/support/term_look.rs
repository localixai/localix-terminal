//! How a terminal's view looks as the person's settings and the system's display options
//! change, in a real window: more contrast draws its text in other colours, less transparency
//! lays its own ground under it where the window has glass (clear again after), as it always is
//! on Linux; a larger font from Settings is drawn at once in a smaller grid, a long line wrapped
//! again to fit; the light and the dark theme are drawn as each is chosen.

use llxterm::browser::{Browser, TermAnswer, TermProbe};
use serde_json::json;

use super::terminal_view::{opened, seen_in};

/// The terminal's id: apart from the other scenes' and the desk's.
const ID: u32 = 911;

fn pixels(browser: &Browser) -> Vec<u8> {
    match browser.term_probe(ID, TermProbe::Pixels) {
        Some(TermAnswer::Pixels(picture)) => picture.rgba,
        other => panic!("the view drew no picture: {other:?}"),
    }
}

fn probe(browser: &Browser, probe: TermProbe) {
    assert_eq!(browser.term_probe(ID, probe), Some(TermAnswer::Done));
}

/// The pixels once the view drew `after` more frames than `before` said.
fn drawn_after(browser: &Browser, before: u64) -> Vec<u8> {
    seen_in(browser, ID, |seen| seen.drawn > before);
    pixels(browser)
}

pub(crate) fn scenario(browser: &Browser) {
    let long = "x".repeat(100);
    let session = opened(
        browser,
        ID,
        &format!("printf '\\033[90mgrey words\\033[0m\\n{long}END\\nREADY\\n'; exec sleep 60"),
    );
    let first = seen_in(browser, ID, |seen| {
        seen.rows.iter().any(|row| row.trim() == "READY")
    });
    assert!(
        first.columns > 103,
        "the pane is too narrow for the check: {first:#?}"
    );
    // The ground under an empty cell: clear over the window's glass on macOS; Linux has no
    // glass, and the view lays a ground of its own (`acts::look`).
    let glass = cfg!(target_os = "macos");
    let ground = if glass { 0 } else { 255 };
    let plain = pixels(browser);
    let corner = plain.len() - 4;
    assert_eq!(plain[corner + 3], ground, "the ground is not the window's");

    // More contrast: the grey words drawn in another colour, the ground as it was.
    probe(
        browser,
        TermProbe::Display {
            more: true,
            solid: !glass,
        },
    );
    let more = drawn_after(browser, first.drawn);
    assert_ne!(more, plain, "more contrast drew the same");
    assert_eq!(more[corner + 3], ground);
    // Less transparency, where there is glass: the terminal's own ground, solid; and clear
    // again without it.
    if glass {
        let drawn = seen_in(browser, ID, |_| true).drawn;
        probe(
            browser,
            TermProbe::Display {
                more: false,
                solid: true,
            },
        );
        let solid = drawn_after(browser, drawn);
        assert_eq!(
            solid[corner + 3],
            255,
            "less transparency left the ground clear"
        );
    }
    let drawn = seen_in(browser, ID, |_| true).drawn;
    probe(
        browser,
        TermProbe::Display {
            more: false,
            solid: !glass,
        },
    );
    assert_eq!(drawn_after(browser, drawn)[corner + 3], ground);

    // A larger font from Settings, drawn at once: fewer columns, the long line wrapped again.
    probe(browser, TermProbe::Settings(json!({"font_size": 20})));
    let larger = seen_in(browser, ID, |seen| seen.columns < first.columns);
    let wrapped = seen_in(browser, ID, |seen| {
        seen.columns == larger.columns
            && seen.rows.iter().any(|row| row.trim_end().ends_with("xEND"))
            && !seen
                .rows
                .iter()
                .any(|row| row.trim_end() == format!("{long}END"))
    });
    assert!(wrapped.lines < first.lines, "{wrapped:#?}");
    probe(browser, TermProbe::Settings(json!({"font_size": 13})));
    seen_in(browser, ID, |seen| seen.columns == first.columns);
    seen_in(browser, ID, |seen| {
        seen.rows
            .iter()
            .any(|row| row.trim_end() == format!("{long}END"))
    });

    // The light theme and the dark one, each drawn as it is chosen.
    probe(browser, TermProbe::Settings(json!({"theme": "light"})));
    seen_in(browser, ID, |seen| !seen.dark);
    probe(browser, TermProbe::Settings(json!({"theme": "dark"})));
    seen_in(browser, ID, |seen| seen.dark);
    session.close();
}
