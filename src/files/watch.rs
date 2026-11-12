//! A folder's changes heard from the system as they happen: the kernel's own notices (FSEvents
//! on macOS, inotify on Linux), never a timer that looks again. Nothing runs while nothing
//! changes. Only the folders listened to are told of, each once however many of its entries
//! changed together; what the system could not keep count of is told as everything.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
#[path = "fsevents.rs"]
mod system;
#[cfg(not(target_os = "macos"))]
#[path = "inotify.rs"]
mod system;

/// What the system said changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// An entry came, went or was renamed in each of these folders, all of them listened to.
    Dirs(Vec<PathBuf>),
    /// The system lost count (too much at once, the root moved): every folder is read again.
    All,
}

/// Who is told of a change: called on the watch's own thread, so it only hands the news on.
type Tell = Box<dyn Fn(Change) + Send + Sync>;

/// The changes under one root, heard until it is dropped.
pub struct Watch(system::Stream);

impl Watch {
    /// The folders under `root` (its real path) watched, `heard` told of each change in one
    /// that is listened to; none is until [`Watch::listen`] names them.
    pub fn new(root: &Path, heard: impl Fn(Change) + Send + Sync + 'static) -> io::Result<Self> {
        system::Stream::new(root, Box::new(heard)).map(Self)
    }

    /// The folders listened to from now on are `dirs`, and no other.
    pub fn listen(&self, dirs: &BTreeSet<PathBuf>) {
        self.0.listen(dirs);
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
