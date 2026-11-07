//! The client's side of the socket: find the server, start it when a call needs one (the app,
//! through LaunchServices, when this program is the app's; else this program as `serve`), say
//! `initialize`, and keep a [`Link`] to ask on. The CLI and `--said --follow` are its clients.

mod link;
mod params;

use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::{Duration, Instant};

use std::sync::Arc;

use serde_json::{Value, json};

pub use link::{Answer, Error, Link, Notes};
pub use params::{Caller, environment, window_params};

use crate::wire::PROTOCOL;

/// Whether a call may start the app.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Start it if nothing answers.
    Spawn,
    /// Never: ending tabs or fetching comments must not bring the app up.
    Never,
}

/// How long a started app has to listen.
const READY: Duration = Duration::from_secs(20);
/// How long the handshake may take.
const SHAKE: Duration = Duration::from_secs(10);

/// What the app said of itself.
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
    /// The app, or `None` when nothing listens and `start` is `Never`. `notes` hears the
    /// app's notifications.
    pub fn connect(start: Start, notes: &Notes) -> Result<Option<Self>, String> {
        let socket = crate::paths::socket();
        if let Ok(stream) = UnixStream::connect(&socket) {
            return Self::over(stream, notes).map(Some);
        }
        if start == Start::Never {
            return Ok(None);
        }
        let exited = reap(start_server()?)?;
        let deadline = Instant::now() + READY;
        loop {
            if let Ok(stream) = UnixStream::connect(&socket) {
                return Self::over(stream, notes).map(Some);
            }
            // One that lost the race for the lock ends well; one that failed says why.
            if let Ok(status) = exited.try_recv()
                && !status.success()
            {
                return Err(format!(
                    "the server could not start ({status}): {}",
                    log_tail()
                ));
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "the server did not start within {}s (see {})",
                    READY.as_secs(),
                    crate::paths::log().display()
                ));
            }
            crate::backoff(crate::BACKOFF);
        }
    }

    /// The handshake over `stream`.
    pub fn over(stream: UnixStream, notes: &Notes) -> Result<Self, String> {
        // Before anything is sent (the caller's environment goes with a call): whatever
        // listens at the socket's path must be this user's own app.
        if !crate::wire::peer::is_own(&stream) {
            return Err("the socket is another user's: this is not your LLX Desk".into());
        }
        let link = Link::open(stream, Notes::clone(notes))
            .map_err(|e| format!("the server's socket: {e}"))?;
        let hello = json!({
            "protocolVersion": PROTOCOL,
            "clientInfo": {"name": "llx-desk", "version": env!("CARGO_PKG_VERSION"), "build": crate::server::build()},
            "capabilities": {"binary": true},
        });
        let answer = link
            .request("initialize", hello, Some(SHAKE))
            .map_err(|why| match why {
                Error::Foreign(detail) => format!(
                    "another build's server answers, of another protocol: quit LLX Desk (`llx-desk quit`) and call again ({detail})"
                ),
                Error::Broken(_) | Error::Timeout => {
                    format!("the server did not complete the handshake: {why}")
                }
                Error::Rpc(error) => error.message,
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
}

/// Why a call has no answer.
pub enum Trouble {
    /// No server runs, and the call did not ask for one.
    None,
    /// The server spoke, and said no.
    Refused(String),
    /// The server could not be reached.
    Lost(String),
}

/// `method` asked of the server, once more after a connection that broke under it: the one
/// request every adapter (the CLI, `llx-desk mcp`) makes.
pub fn ask(start: Start, method: &str, params: &Value) -> Result<Answer, Trouble> {
    let notes: Notes = Arc::new(|_, _| {});
    // Every request has a key, so that the one made again is not done twice.
    let mut params = params.clone();
    if let Some(fields) = params.as_object_mut() {
        fields
            .entry("idempotencyKey")
            .or_insert_with(|| json!(crate::token()));
    }
    let mut last = Trouble::None;
    let (mut start, mut asked) = (start, false);
    for _ in 0..2 {
        let client = match Client::connect(start, &notes) {
            Ok(Some(client)) => client,
            Ok(None) if asked => {
                return Err(Trouble::Lost(
                    "LLX Desk went away while this call ran".into(),
                ));
            }
            Ok(None) => return Err(Trouble::None),
            Err(why) => return Err(Trouble::Lost(why)),
        };
        match client.request(method, params.clone()) {
            Ok(answer) => return Ok(answer),
            Err(Error::Rpc(error)) => return Err(Trouble::Refused(error.message)),
            Err(other @ (Error::Broken(_) | Error::Timeout | Error::Foreign(_))) => {
                last = Trouble::Lost(other.to_string());
                // The app took the call, then the connection broke: an app still there answers
                // the retry from what it kept, and one that is gone was quit, not to be started
                // again for a call it may have run.
                (start, asked) = (Start::Never, true);
            }
        }
    }
    Err(last)
}

/// The server started: the app through LaunchServices when this program is the app's (so it
/// runs as the app, with its name and its Dock icon, and one of it), else this program as
/// `serve`, in a session of its own. Either writes its own log (`server::log_to_home`).
fn start_server() -> Result<Child, String> {
    crate::paths::ensure_home().map_err(|e| format!("{}: {e}", crate::paths::home().display()))?;
    let mut server = match crate::bundle::app() {
        Some(app) => launch(&app),
        None => {
            let exe = std::env::current_exe().map_err(|e| format!("cannot find itself: {e}"))?;
            let mut server = Command::new(exe);
            // The server starts terminals too: with what the person set, not the caller's
            // session.
            server.env_clear().envs(params::scrubbed()).arg("serve");
            // SAFETY: the hook calls only setsid, which is async-signal-safe: the server outlives
            // this caller's terminal and its process group.
            unsafe { server.pre_exec(detach) };
            server
        }
    };
    server
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start the server: {e}"))
}

/// `open` asked to start `app` as the server, in the background (it never takes the person's
/// keys), its files where this caller's are (`paths::MOVED_BY`).
fn launch(app: &std::path::Path) -> Command {
    let mut open = Command::new("/usr/bin/open");
    open.arg("-g").arg("-a").arg(app);
    keep_places(&mut open);
    open.arg("--args").arg("serve");
    open
}

/// `open` told this caller's places for the app's files (`paths::MOVED_BY`), which LaunchServices
/// does not pass on by itself: the app it starts keeps its files where the caller's other
/// commands do.
pub(crate) fn keep_places(open: &mut Command) {
    for name in crate::paths::MOVED_BY {
        if let Some(value) = std::env::var_os(name) {
            let mut pair = std::ffi::OsString::from(name);
            pair.push("=");
            pair.push(value);
            open.arg("--env").arg(pair);
        }
    }
}

/// What started the server (`open`, or the server itself) waited for on a thread of its own, so
/// that it is reaped when it ends however long this process lives; its exit status comes on the
/// channel.
pub(crate) fn reap(mut child: Child) -> Result<Receiver<ExitStatus>, String> {
    let (status, exited) = sync_channel(1);
    std::thread::Builder::new()
        .name("server-reaper".into())
        .spawn(move || {
            if let Ok(ended) = child.wait() {
                let _ = status.send(ended);
            }
        })
        .map_err(|e| format!("could not wait for the app: {e}"))?;
    Ok(exited)
}

/// The last line of the app's log: why it did not start. Only its end is read.
fn log_tail() -> String {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let mut tail = Vec::new();
    if let Ok(mut log) = std::fs::File::open(crate::paths::log()) {
        let _ = log.seek(SeekFrom::End(-4096));
        let _ = log.take(4096).read_to_end(&mut tail);
    }
    String::from_utf8_lossy(&tail)
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .unwrap_or("no word in app.log")
        .to_string()
}

/// Between fork and exec: a session of its own.
fn detach() -> std::io::Result<()> {
    // SAFETY: setsid takes nothing and only makes the forked child a session leader.
    match unsafe { libc::setsid() } {
        -1 => Err(std::io::Error::last_os_error()),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
