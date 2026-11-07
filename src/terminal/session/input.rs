//! What is typed into a session, kept in order and written to its PTY by a thread of its own: the
//! window's keys and pastes only queue (the window never waits on a program that is not reading),
//! and a call's input waits until its bytes were taken, so an agent hears when they were not.

use std::fs::File;
use std::io::{self, Write};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::nap;

/// The most bytes that wait to be taken by the program: past it, more input is refused.
const QUEUE_MAX: usize = 1 << 20;
/// A program that takes no byte for this long is not reading: what waits is dropped.
const STALL: Duration = Duration::from_secs(2);

/// A stretch of input that was dropped: its first and one past its last byte (counted from the
/// session's start), and why.
struct Lost {
    from: u64,
    to: u64,
    why: String,
}

/// How many dropped stretches are remembered: the calls waiting on input are few, and the oldest
/// stretch is the one whose caller has been told long ago.
const LOST_MAX: usize = 64;

#[derive(Default)]
struct Queue {
    /// Bytes waiting for the writer, in the order they came.
    bytes: Vec<u8>,
    /// How many bytes were ever queued, and how many of them are done with (written or dropped).
    pushed: u64,
    done: u64,
    /// What was dropped, oldest first: each call that waits looks for its own bytes in it.
    lost: Vec<Lost>,
    /// The last push was refused (and said so): the next refusal of a streak is not said again.
    refusing: bool,
    closed: bool,
}

impl Queue {
    /// `lost` is remembered; a stretch that goes on from the last is one with it.
    fn lose(&mut self, lost: Lost) {
        match self.lost.last_mut() {
            Some(last) if last.to == lost.from && last.why == lost.why => last.to = lost.to,
            _ => {
                if self.lost.len() == LOST_MAX {
                    self.lost.remove(0);
                }
                self.lost.push(lost);
            }
        }
    }
}

/// A session's input: queued by any thread, written by [`Input::drain`].
#[derive(Default)]
pub(super) struct Input {
    queue: Mutex<Queue>,
    wake: Condvar,
}

impl Input {
    /// Queue `bytes` after the rest and return where they lie: the count before and after.
    fn push(&self, bytes: &[u8]) -> Result<(u64, u64), String> {
        let mut queue = crate::lock(&self.queue);
        if queue.closed {
            return Err("the terminal is closed".into());
        }
        let waiting = usize::try_from(queue.pushed - queue.done).unwrap_or(usize::MAX);
        if waiting.saturating_add(bytes.len()) > QUEUE_MAX {
            return Err("the terminal is not taking its input: too much waits".into());
        }
        let start = queue.pushed;
        queue.bytes.extend_from_slice(bytes);
        queue.pushed += bytes.len() as u64;
        let end = queue.pushed;
        drop(queue);
        self.wake.notify_all();
        Ok((start, end))
    }

    /// Queue `bytes` and go on: the window's typing, which has nobody to tell.
    pub(super) fn queue(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let pushed = self.push(bytes);
        let mut queue = crate::lock(&self.queue);
        match pushed {
            Ok(_) => queue.refusing = false,
            // Said once for a streak of refusals: a program that is not reading may make a
            // query of its own a thousand times a second.
            Err(why) => {
                if !std::mem::replace(&mut queue.refusing, true) {
                    log::warn!("terminal input dropped: {why}");
                }
            }
        }
    }

    /// Queue `bytes` and wait until the program has taken them; an error when it did not.
    pub(super) fn send(&self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let (start, end) = self.push(bytes)?;
        let mut queue = crate::lock(&self.queue);
        while queue.done < end && !queue.closed {
            queue = crate::wait_timeout(&self.wake, queue, Duration::from_secs(1));
        }
        let lost = (queue.lost.iter()).find(|lost| end > lost.from && start < lost.to);
        match lost {
            Some(lost) => Err(lost.why.clone()),
            None if queue.done < end => Err("the terminal closed before it took the input".into()),
            None => Ok(()),
        }
    }

    /// No more input is taken, and the writer ends once what waits is written.
    pub(super) fn close(&self) {
        crate::lock(&self.queue).closed = true;
        self.wake.notify_all();
    }

    /// The writer: queued bytes into `writer` until the input is closed. Bytes the program does not
    /// take for `STALL`, or cannot take at all, are dropped with what waits behind them, and the
    /// calls that sent them are told.
    pub(super) fn drain(&self, mut writer: File) {
        loop {
            let batch = {
                let mut queue = crate::lock(&self.queue);
                while queue.bytes.is_empty() {
                    if queue.closed {
                        return;
                    }
                    queue = crate::wait_timeout(&self.wake, queue, Duration::from_secs(1));
                }
                std::mem::take(&mut queue.bytes)
            };
            let (sent, outcome) = write_all(&mut writer, &batch);
            let mut queue = crate::lock(&self.queue);
            queue.done += sent as u64;
            if let Err(why) = outcome {
                let from = queue.done;
                let dropped = batch.len() - sent + queue.bytes.len();
                queue.bytes.clear();
                queue.done += dropped as u64;
                let to = queue.done;
                queue.lose(Lost { from, to, why });
            }
            drop(queue);
            self.wake.notify_all();
        }
    }
}

/// All of `bytes` into the non-blocking `writer`: how many went, and why not all when they did not.
fn write_all(writer: &mut File, bytes: &[u8]) -> (usize, Result<(), String>) {
    let mut sent = 0;
    let mut moved = Instant::now();
    while sent < bytes.len() {
        match writer.write(bytes.get(sent..).unwrap_or_default()) {
            Ok(n) if n > 0 => {
                sent += n;
                moved = Instant::now();
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() != io::ErrorKind::WouldBlock => {
                return (sent, Err(format!("the terminal takes no input ({e})")));
            }
            _ if moved.elapsed() >= STALL => {
                return (
                    sent,
                    Err("the terminal takes no input: the program is not reading it".into()),
                );
            }
            _ => nap(5),
        }
    }
    (sent, Ok(()))
}
