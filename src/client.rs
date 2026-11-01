//! The client's side of the socket: find the daemon, start it when a call needs one, say
//! `initialize`, and keep a [`Link`] to ask on. The CLI, `--said --follow` and the MCP relay are
//! all clients of it.

mod link;
mod params;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub use link::{Answer, Error, Link, Notes};
pub use params::{host_project, window_params};

use crate::wire::PROTOCOL;

/// Whether a call may start the daemon.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Start it if nothing answers.
    Spawn,
    /// Never: ending tabs or fetching comments must not bring a daemon up.
    Never,
}

/// How long a started daemon has to listen.
const READY: Duration = Duration::from_secs(20);
/// How long the handshake may take.
const SHAKE: Duration = Duration::from_secs(10);

/// Why no connection could be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// A daemon of an older protocol answers the socket: none is started beside it.
    Older(String),
    /// Anything else: it did not start, did not answer, or refused.
    Failed(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Older(why) | Self::Failed(why) => f.write_str(why),
        }
    }
}

/// What the daemon said of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub version: String,
    pub build: String,
    pub pid: u64,
}

/// A connection with its handshake done.
pub struct Client {
    pub link: Link,
    pub server: Server,
}

impl Client {
    /// The daemon, or `None` when nothing listens and `start` is `Never`. `notes` hears the
    /// daemon's notifications.
    pub fn connect(start: Start, notes: &Notes) -> Result<Option<Self>, Trouble> {
        let socket = crate::paths::socket();
        if let Ok(stream) = UnixStream::connect(&socket) {
            return Self::over(stream, notes).map(Some);
        }
        if start == Start::Never {
            return Ok(None);
        }
        let exited = reap(spawn().map_err(Trouble::Failed)?).map_err(Trouble::Failed)?;
        let deadline = Instant::now() + READY;
        loop {
            if let Ok(stream) = UnixStream::connect(&socket) {
                return Self::over(stream, notes).map(Some);
            }
            // One that lost the race for the lock ends well; one that failed says why.
            if let Ok(status) = exited.try_recv()
                && !status.success()
            {
                return Err(Trouble::Failed(format!(
                    "the daemon could not start ({status}): {}",
                    log_tail()
                )));
            }
            if Instant::now() >= deadline {
                return Err(Trouble::Failed(format!(
                    "the daemon did not start within {}s (see {})",
                    READY.as_secs(),
                    crate::paths::home().join("daemon.log").display()
                )));
            }
            backoff();
        }
    }

    /// The handshake over `stream`.
    pub fn over(stream: UnixStream, notes: &Notes) -> Result<Self, Trouble> {
        let link = Link::open(stream, Notes::clone(notes))
            .map_err(|e| Trouble::Failed(format!("the daemon's socket: {e}")))?;
        let hello = json!({
            "protocolVersion": PROTOCOL,
            "clientInfo": {"name": "llx-desk", "version": env!("CARGO_PKG_VERSION"), "build": crate::daemon::build()},
            "capabilities": {"binary": true},
        });
        let answer = link
            .request("initialize", hello, Some(SHAKE))
            .map_err(|why| match why {
                Error::Foreign(detail) => Trouble::Older(older(&detail)),
                Error::Broken(_) | Error::Timeout => {
                    Trouble::Failed(format!("the daemon did not complete the handshake: {why}"))
                }
                Error::Rpc(error) => Trouble::Failed(error.message),
            })?;
        let info = &answer.value["serverInfo"];
        let text = |name: &str| info[name].as_str().unwrap_or_default().to_string();
        let server = Server {
            version: text("version"),
            build: text("build"),
            pid: info["pid"].as_u64().unwrap_or(0),
        };
        Ok(Self { link, server })
    }

    /// `method` asked, waiting for the answer.
    pub fn request(&self, method: &str, params: Value) -> Result<Answer, Error> {
        self.link.request(method, params, None)
    }

    /// `method` asked, waiting up to `limit` for the answer.
    pub fn request_within(
        &self,
        method: &str,
        params: Value,
        limit: Duration,
    ) -> Result<Answer, Error> {
        self.link.request(method, params, Some(limit))
    }
}

/// What to say when the handshake gets no protocol-2 answer: an older daemon is running, and no
/// second one is started beside it.
fn older(detail: &str) -> String {
    let pid = std::fs::read_to_string(crate::paths::lock()).unwrap_or_default();
    format!(
        "a daemon of an older protocol is running (pid {}): `llx-desk serve stop` ends it, and the next call starts this build's ({detail})",
        pid.trim()
    )
}

/// An older daemon's `status` or `stop`, in its own protocol, so the new build can end it.
pub fn legacy(op: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(crate::paths::socket()).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(SHAKE))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{{\"op\":\"{op}\"}}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let said: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    Ok(said["text"].as_str().unwrap_or_default().to_string())
}

/// `llx-desk serve`, detached: its own session, no terminal, its output in the home's log.
fn spawn() -> Result<Child, String> {
    let home = crate::paths::ensure_home()
        .map_err(|e| format!("{}: {e}", crate::paths::home().display()))?;
    let exe = std::env::current_exe().map_err(|e| format!("cannot find itself: {e}"))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("daemon.log"))
        .map_err(|e| format!("daemon.log: {e}"))?;
    let err = log.try_clone().map_err(|e| format!("daemon.log: {e}"))?;
    let mut daemon = Command::new(exe);
    daemon
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err));
    // SAFETY: the hook calls only setsid, which is async-signal-safe: the daemon outlives this
    // caller's terminal and its process group.
    unsafe { daemon.pre_exec(detach) };
    // Not waited for to its end: it is a daemon, reaped by init once this process is gone.
    daemon
        .spawn()
        .map_err(|e| format!("could not start the daemon: {e}"))
}

/// The daemon waited for on a thread of its own, so that it is reaped when it ends however long
/// this process lives (a relay outlives many daemons); its exit status comes on the channel.
fn reap(mut daemon: Child) -> Result<Receiver<ExitStatus>, String> {
    let (status, exited) = sync_channel(1);
    std::thread::Builder::new()
        .name("daemon-reaper".into())
        .spawn(move || {
            if let Ok(ended) = daemon.wait() {
                let _ = status.send(ended);
            }
        })
        .map_err(|e| format!("could not wait for the daemon: {e}"))?;
    Ok(exited)
}

/// The last line of the daemon's log: why it did not start.
fn log_tail() -> String {
    let log = std::fs::read_to_string(crate::paths::home().join("daemon.log")).unwrap_or_default();
    log.lines()
        .last()
        .unwrap_or("no word in daemon.log")
        .to_string()
}

#[expect(
    clippy::disallowed_methods,
    reason = "a backoff while the daemon starts: there is no event to wait on"
)]
fn backoff() {
    std::thread::sleep(Duration::from_millis(50));
}

/// Between fork and exec: a session of its own.
fn detach() -> std::io::Result<()> {
    // SAFETY: setsid takes nothing and only makes the forked child a session leader.
    match unsafe { libc::setsid() } {
        -1 => Err(std::io::Error::last_os_error()),
        _ => Ok(()),
    }
}
