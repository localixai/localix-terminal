//! Tests of `queries`: a program's fixed queries found in its output, read by read, and answered
//! once each, by a session with no page.

use super::super::{DEFAULT_WAIT, Session, Until};
use super::{Queries, Query, replies};

fn start(command: &str) -> Session {
    Session::start(command, &std::env::temp_dir(), None, None).unwrap()
}

/// Every query found in `reads`, fed one after another.
fn found(reads: &[&[u8]]) -> Vec<Query> {
    let mut queries = Queries::default();
    reads.iter().flat_map(|read| queries.asked(read)).collect()
}

#[test]
fn a_cursor_query_split_across_two_reads_is_answered_once() {
    let mut queries = Queries::default();
    assert!(queries.asked(b"prompt\x1b[").is_empty());
    assert_eq!(queries.asked(b"6n more"), [Query::Cursor]);
    // Byte by byte, then two in one read: each found once.
    let asked: usize = (b"\x1b[6n".iter())
        .map(|byte| queries.asked(&[*byte]).len())
        .sum();
    assert_eq!(asked, 1);
    assert_eq!(queries.asked(b"\x1b[6n\x1b[6n").len(), 2);
    assert!(
        queries.asked(b"6n").is_empty(),
        "the end of one is no new one"
    );
}

#[test]
fn the_terminals_name_and_version_are_asked_either_way_xterm_reads_it() {
    assert_eq!(found(&[b"\x1b[>q"]), [Query::Version]);
    assert_eq!(found(&[b"a\x1b[>0qb"]), [Query::Version]);
    // Split byte by byte: found once, the shorter form inside the longer never twice.
    let bytes = b"\x1b[>0q";
    let reads: Vec<&[u8]> = bytes.chunks(1).collect();
    assert_eq!(found(&reads), [Query::Version]);
    let bytes = b"\x1b[>q";
    let reads: Vec<&[u8]> = bytes.chunks(1).collect();
    assert_eq!(found(&reads), [Query::Version]);
}

#[test]
fn other_queries_are_not_these() {
    // Another XTVERSION-like parameter, a private cursor query, the cursor's shape (DECSCUSR)
    // and the secondary device attributes are not asked here.
    for bytes in [
        &b"\x1b[>1q"[..],
        b"\x1b[?6n",
        b"\x1b[2 q",
        b"\x1b[>c",
        b"\x1b[q",
    ] {
        assert!(found(&[bytes]).is_empty(), "{bytes:?}");
    }
}

#[test]
fn queries_are_answered_in_the_order_they_were_asked() {
    assert_eq!(
        found(&[b"\x1b[>q\x1b[6", b"n\x1b[>0q"]),
        [Query::Version, Query::Cursor, Query::Version]
    );
}

#[test]
fn the_name_and_version_are_answered_as_one_device_control_string() {
    let version = env!("CARGO_PKG_VERSION");
    let answered = replies(&[Query::Version, Query::Cursor], (3, 7));
    let expected = format!("\x1bP>|llxterm {version}\x1b\\\x1b[3;7R");
    assert_eq!(String::from_utf8(answered).unwrap(), expected);
}

#[test]
fn a_flood_of_queries_is_answered_only_so_often() {
    let flood = b"\x1b[6n\x1b[>q".repeat(100);
    let asked = found(&[&flood]);
    assert_eq!(asked.len(), 200);
    let answered = String::from_utf8(replies(&asked, (1, 1))).unwrap();
    assert_eq!(
        answered.matches("\x1b[").count() + answered.matches("\x1bP").count(),
        8
    );
}

#[test]
fn a_program_asking_for_the_cursor_is_answered_by_the_session() {
    // Raw, so the reply is not echoed: the program reads its six bytes and shows them.
    let session = start(
        r#"sh -c 'stty raw -echo; printf "\033[6n"; dd bs=1 count=6 2>/dev/null | od -An -c; sleep 20'"#,
    );
    let until = session.wait(Some("1   ;   1   R"), 0, DEFAULT_WAIT);
    assert_eq!(until, Until::Matched, "{}", session.text());
}

#[test]
fn a_program_asking_which_terminal_it_is_in_is_told_by_the_session_with_no_page() {
    // XTVERSION: the reply is a device control string; its escapes are shown as `E` here.
    let version = env!("CARGO_PKG_VERSION");
    let reply = format!("EP>|llxterm {version}E\\");
    let session = start(&format!(
        r#"sh -c 'stty raw -echo; printf "\033[>q"; dd bs=1 count={} 2>/dev/null | tr "\033" E; printf "|end"; sleep 20'"#,
        reply.len()
    ));
    let until = session.wait(Some(&format!("{reply}|end")), 0, DEFAULT_WAIT);
    assert_eq!(until, Until::Matched, "{}", session.text());
}
