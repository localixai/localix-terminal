//! Tests of `clip`: a program's copy (OSC 52) is kept as its text when it is text and no longer
//! than a mebibyte, once; a read of the pasteboard is never answered; and the bell is heard once,
//! never from a sequence that only ends with BEL.

use base64::Engine;

use super::super::Core;
use super::COPIED_MAX;

fn core() -> Core {
    match Core::new(5, 40) {
        Ok(core) => core,
        Err(why) => panic!("a core: {why}"),
    }
}

fn copy(text: &[u8]) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    format!("\x1b]52;c;{encoded}\x07").into_bytes()
}

#[test]
fn a_programs_copy_is_kept_once_as_its_text() {
    let mut core = core();
    core.feed(&copy("Привет, terminal".as_bytes()));
    assert_eq!(core.program_copy().as_deref(), Some("Привет, terminal"));
    assert_eq!(core.program_copy(), None, "taken once");
    assert_eq!(core.replies(), b"", "a copy is not answered");
}

#[test]
fn a_copy_past_a_mebibyte_or_not_text_is_dropped() {
    let mut core = core();
    core.feed(&copy(&vec![b'a'; COPIED_MAX + 1]));
    assert_eq!(core.program_copy(), None, "too long");
    core.feed(&copy(&vec![b'a'; COPIED_MAX]));
    assert_eq!(core.program_copy().map(|text| text.len()), Some(COPIED_MAX));
    core.feed(&copy(&[0xff, 0xfe, 0x00]));
    assert_eq!(core.program_copy(), None, "not text");
}

#[test]
fn a_read_of_the_pasteboard_is_never_answered() {
    let mut core = core();
    core.feed(b"\x1b]52;c;?\x07");
    assert_eq!(core.replies(), b"");
    assert_eq!(core.program_copy(), None);
}

#[test]
fn the_bell_is_heard_once() {
    let mut core = core();
    assert!(!core.bell_rang());
    core.feed(b"\x1b]0;a title\x07");
    assert!(!core.bell_rang(), "BEL that ends a sequence is no bell");
    core.feed(b"done\x07\x07");
    assert!(core.bell_rang());
    assert!(!core.bell_rang(), "taken once");
}
