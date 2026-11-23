//! What a terminal's program says beyond its text, in a real window: its colours (16, 256 and
//! 24-bit) drawn as asked; its links (an address in its output, and one it gave with OSC 8) lit
//! under the pointer while Command is held, a Command-click asking where it opens in the window's
//! dialog (a tab, the default browser, or nowhere) unless Settings chose a tab; its copy
//! (OSC 52) kept off the pasteboard while the terminal does not have the person's keys (a test
//! run is never the app in front), and a read of the pasteboard never answered; its bell news
//! for the person, who is not looking, from a terminal in sight or out of it; ⌘K, which leaves a
//! full-screen program's screen as it is and types nothing into it; and the system's secure
//! input, on while a program reads what it does not show, with no key pressed for it.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use llxterm::browser::{Asked, Browser, Heard, Pointing, TermAnswer, TermProbe};
use llxterm::surface::Surface;
use serde_json::{Value, json};

use super::terminal_view::{COMMAND, opened, seen_in};

/// The terminals' ids: apart from the other scenes' and the desk's.
const COLOURS: u32 = 902;
const LINKS: u32 = 903;
#[cfg(target_os = "macos")]
const TOLD: u32 = 904;
/// A terminal that rings out of sight, behind the one in front.
const UNSEEN: u32 = 920;
const FRONT: u32 = 921;
#[cfg(target_os = "macos")]
const SECRET: u32 = 922;

/// The pointer at a cell of view `id`, as `pointing` says, with `flags` held.
fn point(browser: &Browser, id: u32, pointing: Pointing, (column, row): (u16, u16), flags: usize) {
    let probe = TermProbe::Pointer {
        pointing,
        column,
        row,
        flags,
    };
    assert_eq!(browser.term_probe(id, probe), Some(TermAnswer::Done));
}

/// Until `want` holds, within ten seconds.
fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn scenario(browser: &Browser) {
    colours(browser);
    links(browser);
    unseen_bell(browser);
    #[cfg(target_os = "macos")]
    told(browser);
    #[cfg(target_os = "macos")]
    secure_input(browser);
}

/// A bell rung in a terminal out of sight, which draws nothing: the person hears of it all the
/// same, from its session.
fn unseen_bell(browser: &Browser) {
    let rang = Arc::new(Mutex::new(0));
    let into = Arc::clone(&rang);
    let key = format!("t{UNSEEN}");
    browser.listen(move |tab, heard| {
        if tab == key && heard == Heard::Rang {
            *into.lock().unwrap() += 1;
        }
    });
    // It rings once it is told to, behind the terminal opened after it.
    let unseen = opened(browser, UNSEEN, "read line; printf '\\a'; exec sleep 60");
    let front = opened(browser, FRONT, "printf 'in front\\n'; exec sleep 60");
    seen_in(browser, FRONT, |seen| seen.shown);
    seen_in(browser, UNSEEN, |seen| !seen.shown);
    Surface::text(&*unseen, "\r");
    until("the bell of the terminal out of sight heard", || {
        *rang.lock().unwrap() == 1
    });
    browser.listen(|_, _| {});
    front.close();
    unseen.close();
}

/// A program that turns its echo off and reads: secure input is on with no key pressed, and off
/// once the program shows what is typed again, still with none.
#[cfg(target_os = "macos")]
fn secure_input(browser: &Browser) {
    let session = opened(
        browser,
        SECRET,
        "sleep 0.5; stty -echo; printf 'pw:'; sleep 1; stty echo; printf ' back'; exec sleep 60",
    );
    assert_eq!(
        browser.term_probe(SECRET, TermProbe::Focus(true)),
        Some(TermAnswer::Done)
    );
    let asks = |seen: &llxterm::browser::TermSeen| seen.rows.iter().any(|row| row.contains("pw:"));
    seen_in(browser, SECRET, |seen| seen.secure && asks(seen));
    seen_in(browser, SECRET, |seen| {
        !seen.secure && seen.rows.iter().any(|row| row.contains("pw: back"))
    });
    session.close();
}

/// Grounds in a palette colour, one of 256 and one of 24 bits, each drawn in its own colour.
fn colours(browser: &Browser) {
    let session = opened(
        browser,
        COLOURS,
        "printf '\\033[41m    \\033[48;5;202m    \\033[48;2;12;180;240m    \\033[0m\\nREADY\\n'; \
         exec sleep 60",
    );
    seen_in(browser, COLOURS, |seen| {
        seen.rows.iter().any(|row| row.trim() == "READY")
    });
    let Some(TermAnswer::Pixels(picture)) = browser.term_probe(COLOURS, TermProbe::Pixels) else {
        panic!("the terminal's view drew no picture");
    };
    let has = |want: [u8; 3]| {
        picture
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] == 255 && (0..3).all(|at| pixel[at].abs_diff(want[at]) <= 2))
    };
    assert!(
        has([0xff, 0x5f, 0x00]),
        "the 256-colour ground is not drawn"
    );
    assert!(has([12, 180, 240]), "the 24-bit ground is not drawn");
    // The palette's red, the dark look's or the light one's (term_view/theme.rs).
    assert!(
        has([0xff, 0x7b, 0x72]) || has([0xd7, 0x3a, 0x49]),
        "the palette's red ground is not drawn"
    );
    session.close();
}

/// An address in the output, and a link the program gave its own words.
fn links(browser: &Browser) {
    let session = opened(
        browser,
        LINKS,
        "printf 'see https://example.test/a, then\\n\\033]8;;https://osc.test/x\\033\\\\label\
         \\033]8;;\\033\\\\ after\\n'; exec sleep 60",
    );
    seen_in(browser, LINKS, |seen| {
        seen.rows.iter().any(|row| row.trim() == "label after")
    });
    let key = format!("t{LINKS}");
    browser.group(&key, "links");
    let asked = Arc::new(Mutex::new(vec![]));
    let into = Arc::clone(&asked);
    browser.listen(move |_, heard| {
        if let Heard::Asked(Asked::Page(Some(url))) = heard {
            into.lock().unwrap().push(url);
        }
    });

    // Without Command the pointer lights nothing; Command lights the address under it, then
    // the program's link, and letting go of it puts it out.
    point(browser, LINKS, Pointing::Moved, (10, 0), 0);
    seen_in(browser, LINKS, |seen| seen.link.is_none());
    point(browser, LINKS, Pointing::Flags, (10, 0), COMMAND);
    seen_in(browser, LINKS, |seen| {
        seen.link.as_deref() == Some("https://example.test/a")
    });
    point(browser, LINKS, Pointing::Moved, (2, 1), COMMAND);
    seen_in(browser, LINKS, |seen| {
        seen.link.as_deref() == Some("https://osc.test/x")
    });
    point(browser, LINKS, Pointing::Moved, (8, 1), COMMAND);
    seen_in(browser, LINKS, |seen| seen.link.is_none());

    // A click alone opens nothing; a Command-click asks where the link opens: the address,
    // Open in Browser apart, Cancel, and Open in Tab with the keys, all within the card.
    point(browser, LINKS, Pointing::Down, (10, 0), 0);
    point(browser, LINKS, Pointing::Up, (10, 0), 0);
    let question = json!([
        "Open Link?",
        "https://example.test/a",
        ["Open in Browser", "Cancel", "Open in Tab"],
        true,
        "dlg-yes"
    ]);
    command_click(browser);
    assert_eq!(asked_link(browser), question);
    // Cancel opens nothing; Open in Tab opens it in a tab of the terminal's project.
    browser.click("dialog no");
    until("the question gone", || asked_link(browser).is_null());
    std::thread::sleep(Duration::from_millis(200));
    assert!(asked.lock().unwrap().is_empty(), "cancelled, nothing opens");
    command_click(browser);
    assert_eq!(asked_link(browser), question);
    browser.click("dialog yes");
    until("the address asked for as a page", || {
        !asked.lock().unwrap().is_empty()
    });
    assert_eq!(*asked.lock().unwrap(), ["https://example.test/a"]);

    // With Settings › Terminal › Open links on In a Tab, a Command-click opens it at once.
    let tab = TermProbe::Settings(json!({"open_links": "tab"}));
    assert_eq!(browser.term_probe(LINKS, tab), Some(TermAnswer::Done));
    point(browser, LINKS, Pointing::Down, (2, 1), COMMAND);
    point(browser, LINKS, Pointing::Up, (2, 1), COMMAND);
    until("the program's link asked for as a page", || {
        asked.lock().unwrap().len() == 2
    });
    assert!(asked_link(browser).is_null(), "nothing asked");
    assert_eq!(asked.lock().unwrap()[1], "https://osc.test/x");
    let ask = TermProbe::Settings(json!({"open_links": "ask"}));
    assert_eq!(browser.term_probe(LINKS, ask), Some(TermAnswer::Done));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(asked.lock().unwrap().len(), 2);
    browser.listen(|_, _| {});
    session.close();
}

/// A Command-click on the address in the links' terminal, and its question up.
fn command_click(browser: &Browser) {
    point(browser, LINKS, Pointing::Down, (10, 0), COMMAND);
    point(browser, LINKS, Pointing::Up, (10, 0), COMMAND);
    until("the link's question up", || !asked_link(browser).is_null());
}

/// The window's dialog as the strip draws it: its title, its sentence, its buttons' words in
/// their order, whether they fit in the card, and which has the keys; null with none up.
fn asked_link(browser: &Browser) -> Value {
    let read = "(() => { const d = document.getElementById('dlg'); if (!d) return null; \
        const acts = d.querySelector('.acts'); \
        return [d.querySelector('h2').textContent, d.querySelector('#dlg-d').textContent, \
        [...acts.querySelectorAll('button')].map((b) => b.textContent), \
        acts.scrollWidth <= acts.clientWidth, document.activeElement?.id ?? null]; })()";
    browser.peek_strip(read).unwrap_or(Value::Null)
}

/// A program's copy and its read of the pasteboard, its bell, and ⌘K on its full screen. The
/// person's pasteboard is kept, and put back were the window to write it.
#[cfg(target_os = "macos")]
fn told(browser: &Browser) {
    use base64::Engine;

    use super::pasteboard::{Saved, pasted};
    use super::terminal_view::{press_in, read};
    let probe = format!("llxterm display test {}", std::process::id());
    let _kept = Saved::now(&probe);
    assert!(
        !super::panel::active(),
        "a test run is never the app in front"
    );
    let rang = Arc::new(Mutex::new(0));
    let into = Arc::clone(&rang);
    let key = format!("t{TOLD}");
    browser.listen(move |tab, heard| {
        if tab == key && heard == Heard::Rang {
            *into.lock().unwrap() += 1;
        }
    });
    let copy = base64::engine::general_purpose::STANDARD.encode(&probe);
    // Each byte the program reads, in hex on a line of its own, on its full screen; first it
    // copies, asks for the pasteboard, and rings.
    let session = opened(
        browser,
        TOLD,
        &format!(
            "stty raw -echo; printf '\\033]52;c;{copy}\\a\\033]52;c;?\\a\\a\\033[?1049h\\033[HFULL\\r\\n'; \
             exec xxd -p -c 1"
        ),
    );
    seen_in(browser, TOLD, |seen| {
        seen.alternate && seen.rows.first().is_some_and(|row| row.starts_with("FULL"))
    });
    until("the bell heard, the person not looking", || {
        *rang.lock().unwrap() == 1
    });
    assert_ne!(
        pasted(),
        probe,
        "a copy from a window not in front went to the pasteboard"
    );

    // ⌘K types nothing and leaves the full screen as it is; the pasteboard's read was never
    // answered: what the program reads first is the Return typed after.
    press_in(browser, TOLD, (0x28, COMMAND), "k", "k");
    press_in(browser, TOLD, (0x24, 0), "\r", "\r");
    assert_eq!(read(&session, 0, 1), ["0d"]);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(read(&session, 0, 0), ["0d"]);
    seen_in(browser, TOLD, |seen| seen.alternate);
    browser.listen(|_, _| {});
    session.close();
}
