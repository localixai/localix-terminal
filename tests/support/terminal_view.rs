//! A terminal in its tab, drawn by the window itself: its grid follows the pane, it does not
//! take the window's keys as it opens, the keys the system hands its view reach the program as
//! a Mac terminal sends them (Shift+Enter a newline, ⌥← and ⌘← the readline keys, the Option
//! made Meta on its side only), an input method's text stays the view's until it is committed,
//! and the terminal's ground is left clear over the window's glass. The helpers that open a
//! terminal in a tab and read its view are the other terminal scenes' too.

use std::sync::Arc;
use std::time::{Duration, Instant};

use llxterm::browser::{Browser, TermAnswer, TermProbe, TermSeen};
use llxterm::surface::Surface;
use llxterm::terminal::Session;

#[cfg(target_os = "macos")]
/// The panel's id: apart from the desk's terminals, which count from 1.
const ID: u32 = 900;

#[cfg(target_os = "macos")]
/// The system's modifier bits, with the device's bit for the side of each (the left, but for
/// `RIGHT_OPTION`).
const SHIFT: usize = (1 << 17) | 0x02;
#[cfg(target_os = "macos")]
const OPTION: usize = (1 << 19) | 0x20;
#[cfg(target_os = "macos")]
const RIGHT_OPTION: usize = (1 << 19) | 0x40;
pub(crate) const COMMAND: usize = (1 << 20) | 0x08;

/// `command` started in a tab of its own, `id`, drawn by the window as the desk draws one.
pub(crate) fn opened(browser: &Browser, id: u32, command: &str) -> Arc<Session> {
    let session = Session::start(command, &std::env::temp_dir(), None, None);
    let session = Arc::new(session.expect("the program starts"));
    let surface: Arc<dyn Surface> = Arc::clone(&session) as _;
    let panel = browser.term(id, command, surface);
    let wake = panel.waker();
    session.on_changed(move || {
        let _ = wake();
    });
    session.keep_view(Box::new(panel));
    session
}

/// What view `id` drew once `done` holds of it, within a while.
pub(crate) fn seen_in(browser: &Browser, id: u32, done: impl Fn(&TermSeen) -> bool) -> TermSeen {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let read = match browser.term_probe(id, TermProbe::Seen) {
            Some(TermAnswer::Seen(seen)) => Some(seen),
            _ => None,
        };
        if let Some(read) = read.as_ref().filter(|read| done(read)) {
            return read.clone();
        }
        assert!(
            Instant::now() < deadline,
            "the terminal's view shows {read:#?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(target_os = "macos")]
fn seen(browser: &Browser, done: impl Fn(&TermSeen) -> bool) -> TermSeen {
    seen_in(browser, ID, done)
}

#[cfg(target_os = "macos")]
/// A key pressed in view `id`: its virtual key code, the flags held, what it types and what it
/// types with no modifier.
pub(crate) fn press_in(
    browser: &Browser,
    id: u32,
    (code, flags): (u16, usize),
    chars: &str,
    plain: &str,
) {
    let probe = TermProbe::Key {
        code,
        flags,
        chars: chars.into(),
        plain: plain.into(),
    };
    assert_eq!(browser.term_probe(id, probe), Some(TermAnswer::Done));
}

#[cfg(target_os = "macos")]
fn press(browser: &Browser, code: u16, flags: usize, chars: &str) {
    press_in(browser, ID, (code, flags), chars, chars);
}

#[cfg(target_os = "macos")]
/// The bytes the program read since `from`, one in hex a line, once `want` of them came.
pub(crate) fn read(session: &Session, from: usize, want: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let bytes: Vec<String> = (session.text().lines())
            .map(str::trim)
            .filter(|line| line.len() == 2 && line.chars().all(|c| c.is_ascii_hexdigit()))
            .map(String::from)
            .collect();
        if bytes.len() >= from + want || Instant::now() > deadline {
            return bytes.into_iter().skip(from).collect();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn scenario(browser: &Browser) {
    use llxterm::surface::OptionMeta;

    // Each byte the program reads, in hex on a line of its own; nothing echoed. A key typed
    // before `stty` runs is echoed by the terminal's own line discipline, and the program's hex
    // then shares that line: the program says when its terminal is raw, and nothing is typed
    // before it has.
    let session = opened(
        browser,
        ID,
        "stty raw -echo; printf 'raw\\r\\n'; exec xxd -p -c 1",
    );

    // Shown as it opens, its grid the pane's and not the terminal's first, the keys left where
    // they were.
    let first = seen(browser, |seen| {
        seen.shown && seen.columns > 0 && seen.rows.iter().any(|row| row.trim() == "raw")
    });
    assert_ne!(
        (first.columns, first.lines),
        (80, 24),
        "the grid is not the pane's"
    );
    assert!(first.columns >= 40 && first.lines >= 10, "{first:#?}");
    assert!(
        !first.keys,
        "the terminal took the window's keys as it opened"
    );

    // A letter, through the input method; Return; Shift+Return a newline, as a coding agent's
    // prompt reads it.
    press(browser, 0x00, 0, "a");
    assert_eq!(read(&session, 0, 1), ["61"]);
    press(browser, 0x24, 0, "\r");
    press(browser, 0x24, SHIFT, "\r");
    assert_eq!(read(&session, 1, 2), ["0d", "0a"]);

    // A Mac terminal's keys: ⌥← a word back, ⌘← the line's start.
    press(browser, 0x7b, OPTION, "\u{f702}");
    press(browser, 0x7b, COMMAND, "\u{f702}");
    assert_eq!(read(&session, 3, 3), ["1b", "62", "01"]);

    // The left Option made Meta (as Settings makes it): Option+B is ESC b from the left one,
    // and the right one types the layout's ∫. Both, then neither, as Settings says.
    let meta = |meta: OptionMeta| {
        let probe = TermProbe::Meta(meta);
        assert_eq!(browser.term_probe(ID, probe), Some(TermAnswer::Done));
    };
    meta(OptionMeta::Left);
    press_in(browser, ID, (0x0b, OPTION), "∫", "b");
    press_in(browser, ID, (0x0b, RIGHT_OPTION), "∫", "b");
    assert_eq!(read(&session, 6, 5), ["1b", "62", "e2", "88", "ab"]);
    meta(OptionMeta::Right);
    press_in(browser, ID, (0x0b, OPTION), "∫", "b");
    press_in(browser, ID, (0x0b, RIGHT_OPTION), "∫", "b");
    assert_eq!(read(&session, 11, 5), ["e2", "88", "ab", "1b", "62"]);
    meta(OptionMeta::Off);
    let base = 16;

    // What an input method composes is drawn at the cursor and sent to nothing; what it
    // commits is typed.
    let marked = TermProbe::Marked("に".into());
    assert_eq!(browser.term_probe(ID, marked), Some(TermAnswer::Done));
    seen(browser, |seen| seen.marked == "に");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        read(&session, base, 0),
        Vec::<String>::new(),
        "composing text was sent"
    );
    let insert = TermProbe::Insert("日".into());
    assert_eq!(browser.term_probe(ID, insert), Some(TermAnswer::Done));
    assert_eq!(read(&session, base, 3), ["e6", "97", "a5"]);
    seen(browser, |seen| seen.marked.is_empty());

    // The view's rows say what the program printed, and its ground is clear where nothing is
    // drawn: the window's glass shows through.
    seen(browser, |seen| {
        seen.rows.iter().any(|row| row.trim() == "a5")
    });
    let Some(TermAnswer::Pixels(picture)) = browser.term_probe(ID, TermProbe::Pixels) else {
        panic!("the terminal's view drew no picture");
    };
    assert!(picture.width > 0 && picture.height > 0);
    let corner = picture.rgba.len() - 4;
    assert_eq!(
        picture.rgba[corner + 3],
        0,
        "the ground under an empty cell is not clear"
    );

    session.close();
}
