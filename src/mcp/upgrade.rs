//! A new build takes over a running server. The host keeps one MCP server for its whole
//! session, and restarts it neither when the plugin is reloaded nor when the binary is replaced;
//! so the server watches its own file, and when another build is in place, settled and
//! answering, it runs it in its own process (`exec`) on the same pipes. That happens only while
//! no call runs and no part of a message has been read, so the host loses nothing: the new build
//! reads what is waiting, and tells the host the tools may have changed.

use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

/// What tells one build's file from another's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Build {
    dev: u64,
    ino: u64,
    len: u64,
    modified: (i64, i64),
}

fn build_of(path: &Path) -> Option<(Build, SystemTime)> {
    let found = std::fs::metadata(path).ok()?;
    let build = Build {
        dev: found.dev(),
        ino: found.ino(),
        len: found.len(),
        modified: (found.mtime(), found.mtime_nsec()),
    };
    Some((build, found.modified().ok()?))
}

/// The server's own file, and the builds seen there.
pub(super) struct Upgrade {
    exe: PathBuf,
    running: Option<Build>,
    refused: Option<Build>,
    /// How long a new file stays unchanged before it is run.
    settle: Duration,
}

impl Upgrade {
    pub(super) fn new(exe: PathBuf, settle: Duration) -> Self {
        let running = build_of(&exe).map(|(build, _)| build);
        Self {
            exe,
            running,
            refused: None,
            settle,
        }
    }

    /// Whether another build is in place, settled, and answers as this program does. One that
    /// does not answer is said once and never tried again.
    pub(super) fn due(&mut self) -> bool {
        let Some((now, modified)) = build_of(&self.exe) else {
            return false;
        };
        if Some(now) == self.running || Some(now) == self.refused {
            return false;
        }
        if modified.elapsed().map_or(true, |age| age < self.settle) {
            return false;
        }
        if answers(&self.exe) {
            return true;
        }
        self.refused = Some(now);
        crate::out::warn(&format!(
            "llx-desk mcp: the build at {} did not answer; this one goes on",
            self.exe.display()
        ));
        false
    }

    /// The new build run in this process with `args`; it returns only when that failed.
    pub(super) fn take_over(&mut self, args: &[String]) -> io::Error {
        let why = Command::new(&self.exe).args(args).exec();
        // Not tried again for this file.
        self.refused = build_of(&self.exe).map(|(build, _)| build);
        why
    }
}

/// `exe --version` names this program, within a few seconds.
fn answers(exe: &Path) -> bool {
    let Ok(mut child) = Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut said = String::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_string(&mut said);
                }
                return status.success() && said.starts_with("llx-desk ");
            }
            Ok(None) if Instant::now() < deadline => backoff(),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "a backoff while the short check runs: a child's end has no event to wait on"
)]
fn backoff() {
    std::thread::sleep(Duration::from_millis(20));
}

/// The host's input, which calls `idle` each `every` while nothing waits to be read and no part
/// of a message has been: the one moment a take-over loses nothing.
pub(super) struct Waiting<R, F> {
    pub(super) input: R,
    pub(super) every: Duration,
    pub(super) idle: F,
    /// The last byte read ended a line (or nothing was read yet).
    pub(super) between: bool,
}

impl<R: Read + AsRawFd, F: FnMut()> Read for Waiting<R, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.between && !ready(self.input.as_raw_fd(), self.every) {
            (self.idle)();
        }
        let read = self.input.read(buf)?;
        if let Some(last) = buf.get(..read).and_then(<[u8]>::last) {
            self.between = *last == b'\n';
        }
        Ok(read)
    }
}

/// Whether `fd` has something to read (or has ended, or failed: the read says which) within
/// `every`.
fn ready(fd: RawFd, every: Duration) -> bool {
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = libc::c_int::try_from(every.as_millis()).unwrap_or(libc::c_int::MAX);
    // SAFETY: one pollfd that lives through the call, and its count.
    let found = unsafe { libc::poll(&raw mut poll, 1, millis) };
    found != 0
}

#[cfg(test)]
#[path = "upgrade_tests.rs"]
mod tests;
