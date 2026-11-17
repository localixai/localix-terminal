//! A terminal to VoiceOver and to an agent, in a real window: the view is a text area named
//! Terminal whose text is the screen's lines and whose insertion point is on the cursor's line;
//! new output is read out as it comes and the echo of what was typed is not; the picture an
//! agent is given of it is drawn by the window, shown or out of sight; and out of sight the view
//! holds no picture of a row, and shown again it draws them all.

use std::time::Duration;

use llxterm::browser::{Browser, Reader, TermAnswer, TermProbe};

use super::terminal_view::{opened, seen_in};

/// The terminals' ids: apart from the other scenes' and the desk's.
const ID: u32 = 905;
const OTHER: u32 = 906;

fn reader(browser: &Browser) -> Reader {
    match browser.term_probe(ID, TermProbe::Reader) {
        Some(TermAnswer::Reader(reader)) => reader,
        other => panic!("the view answered VoiceOver with {other:?}"),
    }
}

/// What VoiceOver hears of the view once `done` holds of it, within a while.
fn heard(browser: &Browser, done: impl Fn(&Reader) -> bool) -> Reader {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let read = reader(browser);
        if done(&read) {
            return read;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "VoiceOver hears {read:#?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn scenario(browser: &Browser) {
    // A program that prints two lines, then echoes what is typed, line by line.
    let session = opened(browser, ID, "printf 'first line\\nREADY\\n'; exec cat");
    seen_in(browser, ID, |seen| {
        seen.rows.iter().any(|row| row.trim() == "READY")
    });

    // A text area, named, its text the screen's lines; the insertion point on the cursor's line.
    let read = heard(browser, |read| read.value.starts_with("first line\nREADY"));
    assert_eq!(
        (read.role.as_str(), read.label.as_str()),
        ("AXTextArea", "Terminal")
    );
    assert_eq!(read.characters, read.value.encode_utf16().count());
    assert_eq!((read.line, read.line_text.as_str()), (2, ""), "{read:#?}");

    // Typed, its echo is not read out; Return, and the line the program prints back is.
    std::thread::sleep(Duration::from_millis(100));
    let insert = TermProbe::Insert("ab".into());
    assert_eq!(browser.term_probe(ID, insert), Some(TermAnswer::Done));
    let echoed = heard(browser, |read| read.line_text == "ab");
    assert_eq!(echoed.said, "", "the echo of the keys was read out");
    let enter = TermProbe::Key {
        code: 0x24,
        flags: 0,
        chars: "\r".into(),
        plain: "\r".into(),
    };
    assert_eq!(browser.term_probe(ID, enter), Some(TermAnswer::Done));
    heard(browser, |read| read.said == "ab");

    // The picture an agent is given: drawn by the window, as JPEG.
    let shot = browser.term_shot(ID).expect("a picture of the terminal");
    assert!(
        shot.width > 100 && shot.height > 100,
        "{}x{}",
        shot.width,
        shot.height
    );
    assert_eq!(&shot.jpeg[..2], [0xff, 0xd8], "not a JPEG");

    // Out of sight behind another tab: no picture of a row is kept, and an agent still gets one.
    let other = opened(browser, OTHER, "printf 'the other tab\\n'; exec sleep 60");
    seen_in(browser, OTHER, |seen| seen.shown);
    let hidden = seen_in(browser, ID, |seen| !seen.shown);
    assert_eq!((hidden.layers, hidden.bytes), (0, 0), "{hidden:#?}");
    let shot = browser
        .term_shot(ID)
        .expect("a picture of the tab out of sight");
    assert!(shot.width > 100 && shot.height > 100);
    let still = seen_in(browser, ID, |seen| !seen.shown);
    assert_eq!(
        still.bytes, 0,
        "the picture kept rows of a view out of sight"
    );

    // Shown again, every row is drawn anew, as it was.
    browser.act(&format!("t{ID}"), false).unwrap();
    let shown = seen_in(browser, ID, |seen| {
        seen.shown && seen.layers > 0 && seen.bytes > 0
    });
    assert!(
        shown.rows.iter().any(|row| row.trim() == "READY"),
        "{shown:#?}"
    );
    other.close();
    session.close();
}
