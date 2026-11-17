//! What a program asks of the terminal it runs in, kept by the view, in a real window: told of
//! focus as the view gains and loses the keys (1004), its cursor drawn in the shape it set
//! (DECSCUSR), a paste framed as it asked (2004), and on its own screen the wheel turned into
//! arrow keys (1007).

use llxterm::browser::{Browser, Edit, TermAnswer, TermProbe};
use llxterm::surface::Shape;

use super::pasteboard::{Saved, put};
use super::terminal_view::{opened, read, seen_in};

/// The terminals' ids: apart from the other scenes' and the desk's.
const ID: u32 = 909;
const FULL: u32 = 910;

fn hex(text: &str) -> Vec<String> {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

fn probe(browser: &Browser, id: u32, probe: TermProbe) {
    assert_eq!(browser.term_probe(id, probe), Some(TermAnswer::Done));
}

pub(crate) fn scenario(browser: &Browser) {
    // Focus reports, a paste framed, a bar for a cursor; then each byte read, in hex a line.
    let session = opened(
        browser,
        ID,
        "stty raw -echo; printf '\\033[?1004h\\033[?2004h\\033[6 q'; exec xxd -p -c 1",
    );
    seen_in(browser, ID, |seen| seen.shown && seen.shape == Shape::Bar);

    // The keys given to the view and taken from it: the program hears both.
    probe(browser, ID, TermProbe::Focus(true));
    probe(browser, ID, TermProbe::Focus(false));
    let mut told = hex("\x1b[I");
    told.extend(hex("\x1b[O"));
    assert_eq!(read(&session, 0, 6), told, "focus was not reported");
    probe(browser, ID, TermProbe::Focus(true));
    let base = read(&session, 0, 9).len();

    // A paste, framed as the program asked: it knows it was not typed.
    {
        let _kept = Saved::now("pasted");
        put("pasted");
        probe(browser, ID, TermProbe::Edit(Edit::Paste));
        let mut framed = hex("\x1b[200~");
        framed.extend(hex("pasted"));
        framed.extend(hex("\x1b[201~"));
        assert_eq!(read(&session, base, framed.len()), framed);
    }
    session.close();

    // On its own screen, with the wheel as arrows asked for, the wheel types arrow keys.
    let full = opened(
        browser,
        FULL,
        "stty raw -echo; printf '\\033[?1049h\\033[?1007hFULL\\r\\n'; exec xxd -p -c 1",
    );
    seen_in(browser, FULL, |seen| seen.alternate && seen.shown);
    probe(browser, FULL, TermProbe::Wheel(1));
    let up: Vec<String> = read(&full, 0, 3).into_iter().take(3).collect();
    assert!(
        up == hex("\x1b[A") || up == hex("\x1bOA"),
        "the wheel sent {up:?}, not an arrow up"
    );
    full.close();
}
