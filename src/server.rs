//! The app: one per user, holding the window and every owner's tabs, serving protocol v2 over
//! a unix socket. `bind` takes the lock and the socket (so a second app ends before it starts
//! anything); `run` answers connections until a `app/quit`, a thread each (`conn`), a request's
//! panic being that request's error and never the app's end.

mod api;
mod conn;
mod hub;
mod idem;
mod methods;
#[cfg(feature = "test-server")]
mod probes;

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::browser::Browser;
use crate::desk::Desk;
use hub::Hub;
use idem::Idem;

/// Connections served at once; one more is closed at once.
const CONNECTIONS: usize = 64;

/// The lock and the socket an app holds while it runs.
pub struct Bound {
    listener: UnixListener,
    socket: PathBuf,
    /// Held for its lock: closing it releases the lock.
    _lock: File,
}

/// What `bind` found.
pub enum Binding {
    Bound(Bound),
    /// Another app of this user holds the lock.
    Running,
}

/// The process that holds the lock, by the pid it wrote there.
pub fn holder() -> Option<libc::pid_t> {
    let written = std::fs::read_to_string(crate::paths::lock()).ok()?;
    written.trim().parse().ok().filter(|pid| *pid > 0)
}

/// Take the lock (writing the pid into it) and the socket.
pub fn bind() -> io::Result<Binding> {
    crate::paths::ensure_runtime()?;
    crate::paths::ensure_home()?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(crate::paths::lock())?;
    // SAFETY: flock on a descriptor this function owns.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = io::Error::last_os_error();
        return match error.kind() == io::ErrorKind::WouldBlock {
            true => Ok(Binding::Running),
            false => Err(error),
        };
    }
    lock.set_len(0)?;
    io::Write::write_all(&mut &lock, std::process::id().to_string().as_bytes())?;
    let socket = crate::paths::socket();
    // A socket left by an app that died: the lock says nobody serves it.
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(Binding::Bound(Bound {
        listener,
        socket,
        _lock: lock,
    }))
}

/// What every connection shares.
pub struct Shared {
    desk: Arc<Desk>,
    hub: Arc<Hub>,
    idem: Idem,
    /// This build, as `initialize` and `status` say it.
    build: String,
    stopping: AtomicBool,
    socket: PathBuf,
    connections: AtomicUsize,
    next: AtomicU64,
}

impl Shared {
    fn next_connection(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst)
    }

    /// End the app: subscribers are told, and the accept loop wakes to see the flag.
    fn stop(&self) {
        use crate::desk::Events as _;
        self.stopping.store(true, Ordering::SeqCst);
        self.hub
            .publish("app/quitting", None, serde_json::json!({}));
        // Written to every subscriber before the process ends, not left in their queues.
        self.hub.flush();
        // The accept loop wakes on a connection of its own, sees the flag and ends.
        let _ = UnixStream::connect(&self.socket);
    }
}

/// This build: its version and the binary's size and time, so a rebuilt binary of the same
/// version is another build. A test build may name one in `LLX_DESK_BUILD`.
pub fn build() -> String {
    if cfg!(feature = "test-server")
        && let Some(named) = std::env::var("LLX_DESK_BUILD")
            .ok()
            .filter(|b| !b.is_empty())
    {
        return named;
    }
    let stamp = std::env::current_exe()
        .and_then(std::fs::metadata)
        .ok()
        .and_then(|meta| {
            let age = meta
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?;
            Some(format!("{}-{}", meta.len(), age.as_secs()))
        })
        .unwrap_or_default();
    format!("{}+{stamp}", env!("CARGO_PKG_VERSION"))
}

/// The user id of the process at the other end of `stream`, when the system says.
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: getpeereid writes the peer's ids into the two variables, which outlive the call.
    let got = unsafe { libc::getpeereid(stream.as_raw_fd(), &raw mut uid, &raw mut gid) };
    (got == 0).then_some(uid)
}

/// The user id of the process at the other end of `stream`, when the system says.
#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut size = libc::socklen_t::try_from(std::mem::size_of::<libc::ucred>()).ok()?;
    // SAFETY: getsockopt writes at most `size` bytes of the peer's credentials into `cred`.
    let got = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &raw mut size,
        )
    };
    (got == 0).then_some(cred.uid)
}

fn own_uid() -> u32 {
    // SAFETY: geteuid takes nothing and cannot fail.
    unsafe { libc::geteuid() }
}

/// How the server begins.
#[derive(Debug, Clone, Copy)]
pub struct Begin {
    /// Its pages drawn out of sight.
    pub headless: bool,
    /// The person opened the app: the window comes to them at once, with their shell when it
    /// has no tab.
    pub opened: bool,
}

/// The server's output (its log, a panic's words) into `app.log` in the home when no terminal
/// shows it: started by LaunchServices or by a client, it has nowhere else to go.
pub fn log_to_home() {
    // SAFETY: isatty only asks what descriptor 2 is.
    if unsafe { libc::isatty(2) } == 1 {
        return;
    }
    if crate::paths::ensure_home().is_err() {
        return;
    }
    let opened = File::options()
        .create(true)
        .append(true)
        .open(crate::paths::log());
    let Ok(log) = opened else { return };
    // SAFETY: dup2 points stdout at the log, which is open through the call.
    unsafe { libc::dup2(log.as_raw_fd(), 1) };
    // SAFETY: as above, for stderr.
    unsafe { libc::dup2(log.as_raw_fd(), 2) };
}

/// Serve until the server stops (`app/quit`, or the person quits the app); the exit code.
/// `browser` is the window's handle, none for a server with terminals only.
pub fn run(bound: &Bound, browser: Option<Browser>, begin: Begin) -> i32 {
    let hub = Arc::new(Hub::default());
    let events: Arc<dyn crate::desk::Events> = Arc::<Hub>::clone(&hub);
    // What the desk keeps across restarts; without its database it still works, keeping nothing.
    let store = crate::store::Store::open(&crate::paths::database()).or_else(|why| {
        log::warn!(
            "desk: its database could not be opened, nothing is kept across restarts: {why}"
        );
        crate::store::Store::memory()
    });
    let store = match store {
        Ok(store) => store,
        Err(why) => {
            log::error!("desk: no database at all: {why}");
            return 1;
        }
    };
    let desk = Desk::new(
        browser,
        begin.headless,
        crate::paths::shots(),
        events,
        Arc::new(store),
    );
    let shared = Arc::new(Shared {
        desk,
        hub,
        idem: Idem::default(),
        build: build(),
        stopping: AtomicBool::new(false),
        socket: bound.socket.clone(),
        connections: AtomicUsize::new(0),
        next: AtomicU64::new(1),
    });
    // The person quitting the app stops the server as `app/quit` does.
    let quitting = Arc::downgrade(&shared);
    shared.desk.on_quit(move || {
        if let Some(shared) = quitting.upgrade() {
            shared.stop();
        }
    });
    if begin.opened {
        shared.desk.reopened();
    }
    for stream in bound.listener.incoming() {
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        // The socket's directory is the user's alone; a process of another user that reached it
        // anyway is still refused.
        if peer_uid(&stream).is_some_and(|uid| uid != own_uid()) {
            conn::refuse(&stream, "this server is another user's");
            continue;
        }
        if shared.connections.fetch_add(1, Ordering::SeqCst) >= CONNECTIONS {
            shared.connections.fetch_sub(1, Ordering::SeqCst);
            conn::refuse(
                &stream,
                &format!("{CONNECTIONS} connections are open: try again"),
            );
            continue;
        }
        let serving = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("desk-conn".into())
            .spawn(move || {
                conn::serve(stream, &serving);
                serving.connections.fetch_sub(1, Ordering::SeqCst);
            });
        if let Err(why) = thread {
            shared.connections.fetch_sub(1, Ordering::SeqCst);
            log::warn!("a connection was dropped (no thread: {why})");
        }
    }
    shared.desk.shutdown();
    let _ = std::fs::remove_file(&bound.socket);
    0
}
