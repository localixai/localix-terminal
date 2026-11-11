//! Tests of `output`: a program's cursor queries found in its output, read by read.

use std::io::{self, Read};

use std::time::Duration;

use super::{Paced, Queries, fill};

#[test]
fn a_cursor_query_split_across_two_reads_is_answered_once() {
    let mut queries = Queries::default();
    assert_eq!(queries.asked(b"prompt\x1b["), 0);
    assert_eq!(queries.asked(b"6n more"), 1);
    // Byte by byte, then two in one read: each found once.
    let asked: usize = (b"\x1b[6n".iter())
        .map(|byte| queries.asked(&[*byte]))
        .sum();
    assert_eq!(asked, 1);
    assert_eq!(queries.asked(b"\x1b[6n\x1b[6n"), 2);
    assert_eq!(queries.asked(b"6n"), 0, "the end of one is no new one");
}

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
