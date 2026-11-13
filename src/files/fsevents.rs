//! macOS: one FSEvents stream over the root, on a queue of its own. The system tells which
//! folders changed, a burst of changes in one folder as one notice; a folder nobody listens to
//! is dropped here, before anyone is woken.

use std::collections::BTreeSet;
use std::ffi::{CStr, OsStr, c_char, c_void};
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Mutex;

use super::{Change, Tell};

/// How long the system gathers changes before it tells of them, in seconds. The first of a
/// quiet stretch is told at once (`NO_DEFER`); this only holds a burst together.
const GATHER: f64 = 0.05;
/// From now on: no change from before the stream began.
const SINCE_NOW: u64 = u64::MAX;
/// The first change after a quiet stretch is told at once, not after `GATHER`.
const NO_DEFER: u32 = 0x2;
/// The root itself moved or went: told as a change, flagged `ROOT_CHANGED`.
const WATCH_ROOT: u32 = 0x4;
/// Flags of a notice that names no one folder: the system dropped some, a folder must be
/// scanned whole, the root changed, a volume came or went.
const LOST: u32 = 0x1 | 0x2 | 0x4 | 0x20 | 0x40 | 0x80;
const UTF8: u32 = 0x0800_0100;

type Ref = *const c_void;

#[repr(C)]
struct Context {
    version: isize,
    info: *mut c_void,
    retain: Ref,
    release: Ref,
    describe: Ref,
}

/// The table an array of Core Foundation objects is made with; only its address is used.
#[repr(C)]
struct Callbacks {
    _opaque: [u8; 0],
}

type Callback = extern "C" fn(Ref, *mut c_void, usize, *mut c_void, *const u32, *const u64);

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn FSEventStreamCreate(
        allocator: Ref,
        callback: Callback,
        context: *const Context,
        paths: Ref,
        since: u64,
        latency: f64,
        flags: u32,
    ) -> *mut c_void;
    fn FSEventStreamSetDispatchQueue(stream: *mut c_void, queue: *mut c_void);
    fn FSEventStreamStart(stream: *mut c_void) -> u8;
    fn FSEventStreamStop(stream: *mut c_void);
    fn FSEventStreamInvalidate(stream: *mut c_void);
    fn FSEventStreamRelease(stream: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeArrayCallBacks: Callbacks;
    fn CFStringCreateWithBytes(
        allocator: Ref,
        bytes: *const u8,
        count: isize,
        encoding: u32,
        external: u8,
    ) -> Ref;
    fn CFArrayCreate(
        allocator: Ref,
        values: *const Ref,
        count: isize,
        callbacks: *const Callbacks,
    ) -> Ref;
    fn CFRelease(object: Ref);
}

unsafe extern "C" {
    fn dispatch_queue_create(label: *const c_char, attributes: Ref) -> *mut c_void;
    fn dispatch_async_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn dispatch_release(object: *mut c_void);
}

/// What the stream's callback reads: the folders listened to, and who is told.
struct Heard {
    listened: Mutex<BTreeSet<PathBuf>>,
    tell: Tell,
}

/// A running stream, its queue, and what its callback reads; all let go of together.
pub(super) struct Stream {
    stream: *mut c_void,
    queue: *mut c_void,
    heard: *mut Heard,
}

// SAFETY: the stream and its queue are the system's own thread-safe objects, used here only to
// stop and release them; `heard` is read by the callback and written through its mutex.
unsafe impl Send for Stream {}
// SAFETY: as above; `listen` takes the mutex.
unsafe impl Sync for Stream {}

impl Stream {
    pub(super) fn new(root: &Path, tell: Tell) -> io::Result<Self> {
        let bytes = root.as_os_str().as_bytes();
        let count = isize::try_from(bytes.len()).map_err(io::Error::other)?;
        // SAFETY: the bytes are valid for the length given; the string made is released below.
        let path = unsafe { CFStringCreateWithBytes(ptr::null(), bytes.as_ptr(), count, UTF8, 0) };
        if path.is_null() {
            return Err(io::Error::other(
                "the folder's path is no text the system reads",
            ));
        }
        // SAFETY: an array of the one string above, which it keeps; the table is the system's.
        let paths = unsafe {
            CFArrayCreate(
                ptr::null(),
                &raw const path,
                1,
                &raw const kCFTypeArrayCallBacks,
            )
        };
        let heard = Box::into_raw(Box::new(Heard {
            listened: Mutex::new(BTreeSet::new()),
            tell,
        }));
        let context = Context {
            version: 0,
            info: heard.cast(),
            retain: ptr::null(),
            release: ptr::null(),
            describe: ptr::null(),
        };
        let flags = NO_DEFER | WATCH_ROOT;
        // SAFETY: the context is copied by the system, and `heard` outlives the stream (`drop`);
        // the paths are valid for the call.
        let stream = unsafe {
            FSEventStreamCreate(
                ptr::null(),
                changed,
                &raw const context,
                paths,
                SINCE_NOW,
                GATHER,
                flags,
            )
        };
        // SAFETY: made above, and the stream holds what it needs of them.
        unsafe { CFRelease(paths) };
        // SAFETY: as above.
        unsafe { CFRelease(path) };
        if stream.is_null() {
            // SAFETY: made by `Box::into_raw` above, and no stream reads it.
            drop(unsafe { Box::from_raw(heard) });
            return Err(io::Error::other("the system gave no stream of file events"));
        }
        // SAFETY: the label is a C string that lives for the call.
        let queue = unsafe { dispatch_queue_create(c"llxterm.files".as_ptr(), ptr::null()) };
        // SAFETY: the stream and the queue are ours, made above.
        unsafe { FSEventStreamSetDispatchQueue(stream, queue) };
        // SAFETY: the stream has its queue.
        let started = unsafe { FSEventStreamStart(stream) } != 0;
        // Dropped where it did not start: the stream, its queue and `heard` are let go of.
        let this = Self {
            stream,
            queue,
            heard,
        };
        if started {
            Ok(this)
        } else {
            Err(io::Error::other("the stream of file events did not start"))
        }
    }

    pub(super) fn listen(&self, dirs: &BTreeSet<PathBuf>) {
        // SAFETY: `heard` lives until `drop`.
        let heard = unsafe { &*self.heard };
        crate::lock(&heard.listened).clone_from(dirs);
    }
}

/// What a stream let go of still holds until a callback under way has returned.
struct Left {
    stream: *mut c_void,
    heard: *mut Heard,
}

impl Drop for Stream {
    /// The stream is stopped here: no callback begins after it. The stream and what its
    /// callback reads are released on its queue, after a callback under way, so the drop
    /// never waits on the queue: it may come in the stream's own callback (its news let go of
    /// the last owner) or under a lock that callback waits for.
    fn drop(&mut self) {
        // SAFETY: the stream is ours, started or not.
        unsafe { FSEventStreamStop(self.stream) };
        // SAFETY: as above; no callback is queued after it.
        unsafe { FSEventStreamInvalidate(self.stream) };
        let left = Box::into_raw(Box::new(Left {
            stream: self.stream,
            heard: self.heard,
        }));
        // SAFETY: the queue is ours and keeps itself until its work ran; `release` takes
        // `left` once.
        unsafe { dispatch_async_f(self.queue, left.cast(), release) };
        // SAFETY: released once; the work above holds the queue until it ran.
        unsafe { dispatch_release(self.queue) };
    }
}

/// On the stream's queue, once a callback under way has returned: the stream and what its
/// callback read are let go of.
extern "C" fn release(left: *mut c_void) {
    // SAFETY: made by `Box::into_raw` in `drop`, and given to this work alone.
    let left = unsafe { Box::from_raw(left.cast::<Left>()) };
    // SAFETY: the stream made in `new`, released once.
    unsafe { FSEventStreamRelease(left.stream) };
    // SAFETY: made by `Box::into_raw` in `new`; no callback runs any more.
    drop(unsafe { Box::from_raw(left.heard) });
}

/// The stream's callback, on its queue: the folders that changed among those listened to.
extern "C" fn changed(
    _stream: Ref,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const u32,
    _ids: *const u64,
) {
    // SAFETY: `info` is the `Heard` the stream was made with, alive until the stream is gone.
    let heard = unsafe { &*info.cast::<Heard>() };
    // A panic must not cross into the system's frames: whatever went wrong, the next change
    // is still heard.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: the system gives `count` flags, valid for the call.
        let flags = unsafe { std::slice::from_raw_parts(flags, count) };
        // SAFETY: and as many pointers to C strings, valid for the call.
        let paths = unsafe { std::slice::from_raw_parts(paths.cast::<*const c_char>(), count) };
        let (mut all, mut dirs) = (false, vec![]);
        {
            let listened = crate::lock(&heard.listened);
            for (flag, path) in flags.iter().zip(paths) {
                if flag & LOST != 0 {
                    all = true;
                    continue;
                }
                // SAFETY: a C string of the system's that lives for the call.
                let path = unsafe { CStr::from_ptr(*path) };
                // A folder's path comes with a slash after it; paths compare by their names.
                let dir = Path::new(OsStr::from_bytes(path.to_bytes()));
                if listened.contains(dir) && !dirs.iter().any(|known| known == dir) {
                    dirs.push(dir.to_path_buf());
                }
            }
        }
        if all {
            (heard.tell)(Change::All);
        } else if !dirs.is_empty() {
            (heard.tell)(Change::Dirs(dirs));
        }
    }));
}
