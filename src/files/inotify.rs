//! Linux: inotify on each folder listened to, read by a thread that sleeps in the kernel until
//! one of them changes. A watch is added as a folder is opened and removed as it is shut.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::{Change, Tell};

/// What is heard of a folder: an entry made, removed, renamed or written, and the folder
/// itself gone. Nothing of a file only read: a search through the folder would drown the rest.
const HEARD: u32 = libc::IN_CREATE
    | libc::IN_MODIFY
    | libc::IN_CLOSE_WRITE
    | libc::IN_DELETE
    | libc::IN_MOVED_FROM
    | libc::IN_MOVED_TO
    | libc::IN_DELETE_SELF
    | libc::IN_MOVE_SELF
    | libc::IN_ONLYDIR;

/// What the reader and `listen` share: the inotify descriptor, the folder each watch is of,
/// and who is told.
struct Shared {
    notify: OwnedFd,
    watched: Mutex<BTreeMap<i32, PathBuf>>,
    tell: Tell,
}

/// The watches, the pipe that ends the reader, and the reader.
pub(super) struct Stream {
    shared: Arc<Shared>,
    stop: OwnedFd,
    reader: Option<JoinHandle<()>>,
}

impl Stream {
    pub(super) fn new(_root: &Path, tell: Tell) -> io::Result<Self> {
        // SAFETY: a plain system call; the descriptor it returns is owned from here.
        let notify = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if notify < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: just returned by the kernel, and owned by nothing else.
        let notify = unsafe { OwnedFd::from_raw_fd(notify) };
        let mut ends = [0; 2];
        // SAFETY: `ends` has room for the two descriptors the call writes.
        if unsafe { libc::pipe2(ends.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: just returned by the kernel, and owned by nothing else.
        let stopped = unsafe { OwnedFd::from_raw_fd(ends[0]) };
        // SAFETY: as above.
        let stop = unsafe { OwnedFd::from_raw_fd(ends[1]) };
        let shared = Arc::new(Shared {
            notify,
            watched: Mutex::new(BTreeMap::new()),
            tell,
        });
        let reading = Arc::clone(&shared);
        let reader = std::thread::Builder::new()
            .name("files".into())
            .spawn(move || read(&reading, &stopped))?;
        Ok(Self {
            shared,
            stop,
            reader: Some(reader),
        })
    }

    pub(super) fn listen(&self, dirs: &BTreeSet<PathBuf>) {
        let notify = self.shared.notify.as_raw_fd();
        let mut watched = crate::lock(&self.shared.watched);
        watched.retain(|watch, dir| {
            let kept = dirs.contains(dir);
            if !kept {
                // SAFETY: a plain system call on our descriptor; a watch already gone fails.
                unsafe { libc::inotify_rm_watch(notify, *watch) };
            }
            kept
        });
        let known: BTreeSet<PathBuf> = watched.values().cloned().collect();
        for dir in dirs.difference(&known) {
            let Ok(path) = CString::new(dir.as_os_str().as_bytes()) else {
                continue;
            };
            // SAFETY: `path` is a C string that lives for the call.
            let watch = unsafe { libc::inotify_add_watch(notify, path.as_ptr(), HEARD) };
            if watch >= 0 {
                watched.insert(watch, dir.clone());
            }
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: one byte from a buffer of one, to our own pipe: it wakes the reader.
        unsafe { libc::write(self.stop.as_raw_fd(), [0u8].as_ptr().cast(), 1) };
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

/// The reader: asleep until a watched folder changes or the stream is dropped, then every
/// notice waiting is read and the folders they name are told of, each once.
fn read(shared: &Shared, stopped: &OwnedFd) {
    let mut bytes = [0u8; 16 * 1024];
    loop {
        let mut waits = [
            libc::pollfd {
                fd: shared.notify.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stopped.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: `waits` holds the two entries the call is told of.
        let ready = unsafe { libc::poll(waits.as_mut_ptr(), 2, -1) };
        let [notify, stop] = waits;
        if stop.revents != 0 {
            return;
        }
        if ready < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return;
        }
        if notify.revents == 0 {
            continue;
        }
        // SAFETY: the buffer is valid for writes of its whole length.
        let got = unsafe { libc::read(notify.fd, bytes.as_mut_ptr().cast(), bytes.len()) };
        let Some(read) = usize::try_from(got).ok().and_then(|got| bytes.get(..got)) else {
            continue;
        };
        if let Some(change) = heard(shared, read) {
            (shared.tell)(change);
        }
    }
}

/// What the notices in `read` say: each is a header (the watch, what happened, a cookie, the
/// length of a name) and that name, which is not needed: the folder is read again whole.
fn heard(shared: &Shared, mut read: &[u8]) -> Option<Change> {
    const HEADER: usize = 16;
    let mut watched = crate::lock(&shared.watched);
    let (mut all, mut dirs) = (false, vec![]);
    while let Some((header, rest)) = read.split_at_checked(HEADER) {
        let field = |at: usize| {
            (header.get(at..at + 4))
                .and_then(|four| <[u8; 4]>::try_from(four).ok())
                .map_or(0, u32::from_ne_bytes)
        };
        let (watch, mask, named) = (field(0).cast_signed(), field(4), field(12));
        read = rest
            .get(usize::try_from(named).unwrap_or(usize::MAX)..)
            .unwrap_or_default();
        if mask & libc::IN_Q_OVERFLOW != 0 {
            all = true;
        } else if let Some(dir) = watched.get(&watch)
            && !dirs.contains(dir)
        {
            dirs.push(dir.clone());
        }
        // The kernel let go of the watch (its folder is gone): its number may be given again.
        if mask & libc::IN_IGNORED != 0 {
            watched.remove(&watch);
        }
    }
    match (all, dirs.is_empty()) {
        (true, _) => Some(Change::All),
        (false, false) => Some(Change::Dirs(dirs)),
        (false, true) => None,
    }
}
