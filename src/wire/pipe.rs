//! A connection's one writer: messages are queued as whole byte strings (a response and its
//! bytes are one string), and a thread of its own writes them in order, so nothing lands between
//! a line and its bytes and no lock is held across a write. The queue is bounded: a peer that
//! does not read is cut off, never waited for.

use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::time::Duration;

/// Messages waiting to be written before the peer counts as not reading.
const QUEUED: usize = 256;
/// A write that makes no progress for this long cuts the connection.
const STALLED: Duration = Duration::from_secs(10);

/// The way to a connection's writer.
#[derive(Clone)]
pub struct Outbox(SyncSender<Item>);

/// What the writer is given: bytes to write, or a call to be answered once all before it is.
enum Item {
    Bytes(Vec<u8>),
    Flush(SyncSender<()>),
}

impl Outbox {
    /// Start the writer for `stream`; it ends with the last `Outbox`, or when a write fails (the
    /// connection is then shut, so its reader ends too).
    pub fn spawn(stream: UnixStream, name: &str) -> std::io::Result<Self> {
        let (sender, queue) = sync_channel::<Item>(QUEUED);
        stream.set_write_timeout(Some(STALLED))?;
        std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let mut out = &stream;
                for item in queue {
                    match item {
                        Item::Bytes(bytes) => {
                            if out.write_all(&bytes).and_then(|()| out.flush()).is_err() {
                                break;
                            }
                        }
                        Item::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
                let _ = stream.shutdown(Shutdown::Both);
            })?;
        Ok(Self(sender))
    }

    /// Queue `bytes`, waiting for room; false when the connection is gone.
    pub fn send(&self, bytes: Vec<u8>) -> bool {
        self.0.send(Item::Bytes(bytes)).is_ok()
    }

    /// Queue `bytes` unless the queue is full (a peer that reads too slowly misses it); false
    /// when it was not queued.
    pub fn offer(&self, bytes: Vec<u8>) -> bool {
        match self.0.try_send(Item::Bytes(bytes)) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
        }
    }

    /// Wait until everything queued so far has been written (or the connection is gone).
    pub fn flush(&self) {
        let (done, written) = sync_channel::<()>(1);
        if self.0.send(Item::Flush(done)).is_ok() {
            let _ = written.recv_timeout(STALLED);
        }
    }
}
