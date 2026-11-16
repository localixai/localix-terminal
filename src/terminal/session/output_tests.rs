//! Tests of `output`: the program's output read whole, what follows it paced, and the whole
//! screen sent to a page that fell behind.

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

#[test]
fn a_page_that_fell_behind_is_sent_the_modes_the_program_set_with_the_screen() {
    use base64::Engine;
    let scripts = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let kept = std::sync::Arc::clone(&scripts);
    let painter: super::super::Painter = Box::new(move |script| {
        kept.lock().unwrap().push(script);
        true
    });
    let command = r"printf '\033[?1004h\033[3 qready'; sleep 20";
    let dir = std::env::temp_dir();
    let session = super::super::Session::start(command, &dir, None, Some(painter)).unwrap();
    session.wait(Some("ready"), 0, super::super::DEFAULT_WAIT);
    session.behind();
    let deadline = std::time::Instant::now() + super::super::DEFAULT_WAIT;
    let screen = loop {
        let sent = scripts.lock().unwrap().clone();
        let screen = (sent.iter().rev()).find_map(|script| {
            script
                .strip_prefix("R(\"")?
                .strip_suffix("\")")
                .map(str::to_string)
        });
        if let Some(screen) = screen {
            break base64::engine::general_purpose::STANDARD
                .decode(screen)
                .unwrap();
        }
        assert!(std::time::Instant::now() < deadline, "{sent:?}");
        super::super::nap(20);
    };
    let screen = String::from_utf8(screen).unwrap();
    assert!(
        screen.contains("\x1b[?1004h") && screen.contains("\x1b[3 q"),
        "{screen:?}"
    );
}
