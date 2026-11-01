//! The daemon: one per user, holding the window and every owner's tabs, serving protocol v2 over
//! a unix socket. `bind` takes the lock and the socket (so a second daemon ends before it starts
//! anything); `run` answers connections until a `daemon/stop`, a thread each (`conn`), a request's
//! panic being that request's error and never the daemon's end.

mod conn;
mod hub;
mod idem;
mod methods;
#[cfg(feature = "test-daemon")]
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

/// The lock and the socket a daemon holds while it runs.
pub struct Bound {
    listener: UnixListener,
    socket: PathBuf,
    /// Held for its lock: closing it releases the lock.
    _lock: File,
}

/// What `bind` found.
pub enum Binding {
    Bound(Bound),
    /// Another daemon of this user holds the lock.
    Running,
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
    // A socket left by a daemon that died: the lock says nobody serves it.
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

    /// End the daemon: subscribers are told, and the accept loop wakes to see the flag.
    fn stop(&self) {
        use crate::desk::Events as _;
        self.stopping.store(true, Ordering::SeqCst);
        self.hub
            .publish("daemon/stopping", None, serde_json::json!({}));
        // Written to every subscriber before the process ends, not left in their queues.
        self.hub.flush();
        // The accept loop wakes on a connection of its own, sees the flag and ends.
        let _ = UnixStream::connect(&self.socket);
    }
}

/// This build: its version and the binary's size and time, so a rebuilt binary of the same
/// version is another build. A test build may name one in `LLX_DESK_BUILD`.
pub fn build() -> String {
    if cfg!(feature = "test-daemon")
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

/// Serve until a `daemon/stop`; the exit code. `browser` is the window's handle, none for a
/// daemon with terminals only.
pub fn run(bound: &Bound, browser: Option<Browser>, headless: bool) -> i32 {
    let hub = Arc::new(Hub::default());
    let events: Arc<dyn crate::desk::Events> = Arc::<Hub>::clone(&hub);
    let desk = Desk::new(browser, headless, crate::paths::shots(), events);
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
    for stream in bound.listener.incoming() {
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        if shared.connections.fetch_add(1, Ordering::SeqCst) >= CONNECTIONS {
            shared.connections.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("desk-conn".into())
            .spawn(move || {
                conn::serve(stream, &shared);
                shared.connections.fetch_sub(1, Ordering::SeqCst);
            });
        if let Err(why) = thread {
            log::warn!("a connection was dropped (no thread: {why})");
        }
    }
    shared.desk.shutdown();
    let _ = std::fs::remove_file(&bound.socket);
    0
}
