//! A terminal's output recorded as it came, for development: with `LLXTERM_RECORD` naming a
//! folder, each terminal writes the program's raw output and its resizes there, before anything
//! of it is filtered, as an asciicast v2 file (a header line, then `[seconds, "o", text]` and
//! `[seconds, "r", "COLUMNSxROWS"]` a line each), so a recording can be fed again to an emulator
//! (`tests/core_equivalence.rs`). What a program writes may hold secrets: the file is its user's
//! alone, and nothing records unless the variable is set where the app starts.

use std::fs::File;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The variable that names the folder recordings go to.
const RECORD: &str = "LLXTERM_RECORD";

/// One terminal's recording.
pub(super) struct Record {
    file: File,
    path: PathBuf,
    started: Instant,
    /// The start of a character a read cut in two, kept for the next read: the format holds
    /// text, so a character is written whole.
    cut: Vec<u8>,
}

impl Record {
    /// A recording into the folder `LLXTERM_RECORD` names, of a terminal `rows` by `columns`;
    /// none when the variable is not set, or the file cannot be made (which is logged).
    pub(super) fn from_env(rows: u16, columns: u16) -> Option<Self> {
        let folder = std::env::var_os(RECORD).filter(|folder| !folder.is_empty())?;
        match Self::open(Path::new(&folder), rows, columns) {
            Ok(record) => {
                log::info!("recording a terminal into {}", record.path.display());
                Some(record)
            }
            Err(why) => {
                log::warn!("{RECORD}: no recording in {}: {why}", folder.display());
                None
            }
        }
    }

    /// A new recording in `folder` (made if it is not there), its file named by the time and
    /// this process, readable and writable by its user alone.
    pub(super) fn open(folder: &Path, rows: u16, columns: u16) -> io::Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        std::fs::create_dir_all(folder)?;
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let name = format!(
            "{}-{}-{}.cast",
            since.as_millis(),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = folder.join(name);
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let header = serde_json::json!({
            "version": 2,
            "width": columns,
            "height": rows,
            "timestamp": since.as_secs(),
            "env": {"TERM": super::super::TERM},
        });
        writeln!(file, "{header}")?;
        Ok(Self {
            file,
            path,
            started: Instant::now(),
            cut: Vec::new(),
        })
    }

    /// Records what the program wrote.
    pub(super) fn output(&mut self, bytes: &[u8]) -> io::Result<()> {
        let text = self.text(bytes);
        if text.is_empty() {
            return Ok(());
        }
        self.event("o", &text)
    }

    /// Records that the terminal became `rows` by `columns`.
    pub(super) fn resize(&mut self, rows: u16, columns: u16) -> io::Result<()> {
        self.event("r", &format!("{columns}x{rows}"))
    }

    fn event(&mut self, kind: &str, data: &str) -> io::Result<()> {
        let at = self.started.elapsed().as_secs_f64();
        let line = serde_json::to_string(&(at, kind, data)).map_err(io::Error::other)?;
        writeln!(self.file, "{line}")
    }

    /// `bytes`, after what the last read cut, as text: whole characters only, a byte that
    /// starts none written as U+FFFD, and a character cut at the end kept for the next read.
    fn text(&mut self, bytes: &[u8]) -> String {
        self.cut.extend_from_slice(bytes);
        let mut out = String::with_capacity(self.cut.len());
        let mut rest: &[u8] = &self.cut;
        while !rest.is_empty() {
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    out.push_str(text);
                    rest = &[];
                }
                Err(error) => {
                    let (good, after) = rest.split_at(error.valid_up_to());
                    out.push_str(&String::from_utf8_lossy(good));
                    let Some(bad) = error.error_len() else {
                        rest = after;
                        break;
                    };
                    out.push('\u{fffd}');
                    rest = after.get(bad..).unwrap_or_default();
                }
            }
        }
        let kept = rest.to_vec();
        self.cut = kept;
        out
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

/// Does `what` to the recording in `slot`, if there is one; one that fails is logged and
/// stopped, and the terminal goes on as it would with none.
pub(super) fn keep(slot: &mut Option<Record>, what: impl FnOnce(&mut Record) -> io::Result<()>) {
    let Some(record) = slot.as_mut() else {
        return;
    };
    if let Err(why) = what(record) {
        log::warn!("the recording {} stopped: {why}", record.path.display());
        *slot = None;
    }
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
