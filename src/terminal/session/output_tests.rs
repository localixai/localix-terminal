//! Tests of `output`: the program's output read whole, what follows it paced, a program's
//! queries answered through the core with no view, and a session that closes while its program
//! floods the terminal.

use std::io::{self, Read};

use std::time::Duration;

use super::{Paced, fill};

/// A terminal's end as the system hands it over: a kilobyte a read, then nothing waiting, or
/// closed.
struct Kilobytes {
    left: usize,
    then: Then,
}

/// What a read finds once the output is through.
#[derive(Clone, Copy)]
enum Then {
    NothingWaits,
    Closed,
    Broken,
}

impl Read for Kilobytes {
    fn read(&mut self, room: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return match self.then {
                Then::NothingWaits => Err(io::ErrorKind::WouldBlock.into()),
                Then::Closed => Ok(0),
                Then::Broken => Err(io::ErrorKind::BrokenPipe.into()),
            };
        }
        let n = self.left.min(1024).min(room.len());
        room.iter_mut().take(n).for_each(|byte| *byte = b'x');
        self.left -= n;
        Ok(n)
    }
}

#[test]
fn output_that_waits_is_read_whole_not_a_kilobyte_at_a_time() {
    let mut buffer = vec![0u8; 8192];
    let mut waiting = Kilobytes {
        left: 5000,
        then: Then::NothingWaits,
    };
    assert_eq!(fill(&mut waiting, &mut buffer), (5000, false));
    assert!(buffer.iter().take(5000).all(|byte| *byte == b'x'));
    // More than fits: the buffer's worth now, the rest on the next turn.
    let mut flood = Kilobytes {
        left: 20_000,
        then: Then::NothingWaits,
    };
    assert_eq!(fill(&mut flood, &mut buffer), (8192, false));
    assert_eq!(flood.left, 20_000 - 8192);
}

#[test]
fn the_last_output_of_a_program_that_ended_is_kept() {
    let mut buffer = vec![0u8; 8192];
    let mut closed = Kilobytes {
        left: 1500,
        then: Then::Closed,
    };
    assert_eq!(fill(&mut closed, &mut buffer), (1500, true));
    let mut broken = Kilobytes {
        left: 300,
        then: Then::Broken,
    };
    assert_eq!(fill(&mut broken, &mut buffer), (300, true));
}

#[test]
fn what_follows_output_is_done_at_once_then_at_its_pace() {
    let every = Duration::from_secs(60);
    let mut paced = Paced::default();
    assert!(!paced.due(every), "nothing is owed before any output");
    paced.owed = true;
    assert!(paced.due(every), "the first time is at once");
    paced.owed = true;
    assert!(!paced.due(every), "the next waits for its time");
    assert!(paced.owed, "and stays owed until then");
    assert!(paced.due(Duration::ZERO));
}

#[test]
fn a_session_closed_while_its_program_floods_the_terminal_ends_at_once() {
    // A program that exits with output in its terminal that nobody reads is held in its exit
    // until that output is read: the reader goes on reading after the close, so it never is.
    for _ in 0..20 {
        let session = super::super::Session::start("yes", &std::env::temp_dir(), None, None);
        let session = session.unwrap();
        session.wait(Some("y"), 0, Duration::from_secs(5));
        let (closed, heard) = std::sync::mpsc::sync_channel(1);
        let pid = session.pid();
        std::thread::Builder::new()
            .name("test-close".into())
            .spawn(move || {
                drop(session);
                let _ = closed.send(());
            })
            .unwrap();
        let ended = heard.recv_timeout(Duration::from_secs(10));
        assert!(ended.is_ok(), "closing the session of `yes` ({pid}) hung");
    }
}

fn start(command: &str) -> super::super::Session {
    super::super::Session::start(command, &std::env::temp_dir(), None, None).unwrap()
}

#[test]
fn a_program_asking_for_the_cursor_is_answered_with_no_view() {
    // Raw, so the reply is not echoed: the program reads its six bytes and shows them.
    let session = start(
        r#"sh -c 'stty raw -echo; printf "\033[6n"; dd bs=1 count=6 2>/dev/null | od -An -c; sleep 20'"#,
    );
    let until = session.wait(Some("1   ;   1   R"), 0, super::super::DEFAULT_WAIT);
    assert_eq!(until, super::super::Until::Matched, "{}", session.text());
}

#[test]
fn a_program_asking_which_terminal_it_is_in_is_told_with_no_view() {
    // XTVERSION: the reply is a device control string; its escapes are shown as `E` here.
    let version = env!("CARGO_PKG_VERSION");
    let reply = format!("EP>|llxterm {version}E\\");
    let session = start(&format!(
        r#"sh -c 'stty raw -echo; printf "\033[>q"; dd bs=1 count={} 2>/dev/null | tr "\033" E; printf "|end"; sleep 20'"#,
        reply.len()
    ));
    let until = session.wait(Some(&format!("{reply}|end")), 0, super::super::DEFAULT_WAIT);
    assert_eq!(until, super::super::Until::Matched, "{}", session.text());
}
