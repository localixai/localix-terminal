//! A session as its view sees it, with a real program: the first frame is full and holds what
//! the program wrote; a frame held back by the program (synchronized output) draws the frame from
//! before the hold until it is let go; keys reach the program as it asked for them; a paste is bracketed when the
//! program asked; focus is reported when asked; a find counts and the frame lights its matches.

use std::time::{Duration, Instant};

use crate::surface::{FindStep, Frame, Key, KeyCode, Mods, OptionMeta, Surface};
use crate::terminal::{DEFAULT_WAIT, Session, Until};

fn start(command: &str) -> Session {
    Session::start(command, &std::env::temp_dir(), None, None).unwrap()
}

/// Waits until `seen` holds of the frames drawn, a few seconds at most.
fn frame_until(session: &Session, seen: impl Fn(&Frame) -> bool) -> Frame {
    let started = Instant::now();
    let mut frame = Frame::default();
    while started.elapsed() < Duration::from_secs(5) {
        if session.frame(&mut frame, true) && seen(&frame) {
            return frame;
        }
        crate::backoff(Duration::from_millis(20));
    }
    panic!(
        "never drawn: {:?}",
        frame
            .rows
            .iter()
            .map(|r| r.text.clone())
            .collect::<Vec<_>>()
    );
}

fn shows(frame: &Frame, text: &str) -> bool {
    frame.rows.iter().any(|row| row.text.contains(text))
}

#[test]
fn the_first_frame_is_full_and_shows_the_output() {
    let session = start("printf 'hello from the program'; sleep 20");
    let frame = frame_until(&session, |frame| shows(frame, "hello from the program"));
    assert!(frame.full);
    assert_eq!(
        (frame.columns, frame.lines),
        (crate::terminal::COLUMNS, crate::terminal::ROWS)
    );
}

#[test]
fn a_held_frame_draws_nothing_new_until_it_is_let_go() {
    // The program holds its frame from about 0.3 s to 1.2 s after it starts.
    let started = Instant::now();
    let session = start(
        "printf 'before'; sleep 0.3; printf '\\033[?2026hduring'; sleep 0.9; printf '\\033[?2026l'; sleep 20",
    );
    frame_until(&session, |frame| shows(frame, "before"));
    while started.elapsed() < Duration::from_millis(700) {
        crate::backoff(Duration::from_millis(20));
    }
    let mut frame = Frame::default();
    assert!(
        session.frame(&mut frame, true),
        "held back: drawn as before"
    );
    assert!(shows(&frame, "before") && !shows(&frame, "during"));
    assert!(!session.frame(&mut frame, false), "and nothing new");
    // Agents read the same frame meanwhile, at once.
    assert!(!session.text().contains("during"));
    frame_until(&session, |frame| shows(frame, "beforeduring"));
}

#[test]
fn keys_paste_and_focus_reach_the_program_as_it_asked() {
    // 21 bytes: the focus report (3), the paste bracketed (15) and the cursor key (3).
    let session =
        start("printf '\\033[?2004h\\033[?1004h'; stty raw -echo; head -c 21 | od -c; sleep 20");
    crate::backoff(Duration::from_millis(300));
    session.focus(true);
    session.paste("a\nb");
    let up = Key {
        code: KeyCode::ArrowUp,
        ..Key::default()
    };
    assert!(session.key(&up));
    assert_eq!(
        session.wait(Some("0000025"), 0, DEFAULT_WAIT),
        Until::Matched
    );
    let screen = session.snapshot();
    assert!(
        screen.contains("033   [   I 033   [   2   0   0   ~   a  \\n   b"),
        "{screen}"
    );
    assert!(screen.contains("1   ~ 033   [   A"), "{screen}");
}

#[test]
fn a_find_counts_and_the_frame_lights_its_matches() {
    let session = start("printf 'one needle\\ntwo needle\\n'; sleep 20");
    frame_until(&session, |frame| shows(frame, "two needle"));
    let count = session.find(&FindStep::Needle("NEEDLE".into()));
    assert_eq!((count.at, count.total), (2, 2));
    let frame = frame_until(&session, |frame| frame.matches.len() == 2);
    assert_eq!(frame.current.map(|span| span.y), Some(1));
    session.find(&FindStep::Close);
    frame_until(&session, |frame| frame.matches.is_empty());
}

#[test]
fn a_macs_own_keys_reach_the_program_as_line_editors_read_them() {
    // ⌘← (1 byte), ⌥⌫ (1), ⌥→ (2), and Return from the right Option while it is Meta (2).
    let session = start("stty raw -echo; head -c 6 | od -c; sleep 20");
    crate::backoff(Duration::from_millis(300));
    session.option_meta(OptionMeta::Right);
    let pressed = |code, mods| Key {
        code,
        mods: Mods(mods),
        ..Key::default()
    };
    for (code, mods) in [
        (KeyCode::ArrowLeft, Mods::SUPER),
        (KeyCode::Backspace, Mods::ALT),
        (KeyCode::ArrowRight, Mods::ALT),
        (KeyCode::Enter, Mods::ALT | Mods::ALT_RIGHT),
    ] {
        assert!(session.key(&pressed(code, mods)));
    }
    assert_eq!(
        session.wait(Some("0000006"), 0, DEFAULT_WAIT),
        Until::Matched
    );
    let screen = session.snapshot();
    assert!(screen.contains("001 027 033   f 033  \\r"), "{screen}");
}

#[test]
fn the_view_is_told_as_the_output_changes_and_lets_go_of_what_it_drew() {
    let session = start("printf 'first'; sleep 0.3; printf ' second'; sleep 20");
    let told = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = std::sync::Arc::clone(&told);
    session.on_changed(move || {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    frame_until(&session, |frame| shows(frame, "first second"));
    assert!(told.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    // Out of sight it keeps no render state: the next frame is a full one.
    session.hidden();
    let mut frame = Frame::default();
    assert!(session.frame(&mut frame, false));
    assert!(frame.full && shows(&frame, "first second"));
}

#[test]
fn the_frame_says_how_the_program_ended_once_it_has() {
    let session = start("printf 'last words'; exit 3");
    let running = frame_until(&session, |frame| shows(frame, "last words"));
    assert!(
        running.ended.is_none() || running.ended.as_deref() == Some("(the program exited with 3)")
    );
    // Nothing else changes as it ends: the frame is still new, for the line that says so.
    let ended = frame_until(&session, |frame| frame.ended.is_some());
    assert_eq!(ended.ended.as_deref(), Some("(the program exited with 3)"));
}

#[test]
fn a_frame_held_past_its_second_is_told_as_changed_when_the_second_ends() {
    // The program begins to hold its frame about 0.3 s in, and then writes nothing more.
    let started = Instant::now();
    let session = start("printf 'before'; sleep 0.3; printf '\\033[?2026hduring'; sleep 20");
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Instant>::new()));
    let into = std::sync::Arc::clone(&told);
    session.on_changed(move || into.lock().unwrap().push(Instant::now()));
    frame_until(&session, |frame| shows(frame, "before"));
    // No frame is asked meanwhile: what tells the view must come from the session alone.
    while started.elapsed() < Duration::from_millis(2500) {
        crate::backoff(Duration::from_millis(50));
    }
    let late = started + HOLD_ENDS;
    assert!(
        told.lock().unwrap().iter().any(|at| *at >= late),
        "the view was never told the held frame was let go of"
    );
    let mut frame = Frame::default();
    assert!(session.frame(&mut frame, true));
    assert!(shows(&frame, "beforeduring"));
}

/// When a hold that began 0.3 s after the program started ends at the latest: its second after.
const HOLD_ENDS: Duration = Duration::from_millis(1300);
