//! Test-only helpers: a temporary directory that removes itself, and a stream that keeps the
//! JSON lines written to it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let name = format!(
            "desk-test-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        // The canonical path: macOS's temp directory is a symlink.
        Self(std::fs::canonicalize(&dir).unwrap())
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// Write `content` at `name` under the directory, creating its parents.
    pub(crate) fn file(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A program called `name` in `folder` that prints its arguments: a link to the system's `echo`.
/// Not a script written for the test: macOS assesses each new executable file on its first run,
/// one at a time across the machine, and behind the other new scripts of a parallel run that
/// first run waited for seconds.
pub(crate) fn echo_named(folder: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(folder).unwrap();
    let program = folder.join(name);
    std::os::unix::fs::symlink("/bin/echo", &program).unwrap();
    program
}

/// A stream a writer thread is given in a test: what was written to it, read back as the JSON
/// lines it holds.
#[derive(Clone, Default)]
pub(crate) struct Sink(Arc<Mutex<Vec<u8>>>);

impl Sink {
    pub(crate) fn lines(&self) -> Vec<serde_json::Value> {
        let written = crate::lock(&self.0).clone();
        (String::from_utf8_lossy(&written).lines())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

impl std::io::Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        crate::lock(&self.0).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
