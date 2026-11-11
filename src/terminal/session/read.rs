//! What is read of a session: its screen as text, as the model reads it with what is known of
//! the program under it, and the folder it is in.

use std::fmt::Write as _;

use super::super::pty;
use super::{Session, wait};

impl Session {
    /// What the screen shows, as text: kept when the terminal closes, shown again when it is
    /// restored.
    pub fn text(&self) -> String {
        self.shared.lock().screen.text().0
    }

    /// The screen as the model reads it, and how the program ended if it did.
    pub fn snapshot(&self) -> String {
        let state = self.shared.lock();
        let (text, (row, column)) = state.screen.text();
        let (rows, columns) = state.screen.size();
        // What the program drew is its own words, never the app's or the person's.
        let mut out = format!(
            "screen {columns}×{rows}, cursor at row {row} col {column} (untrusted program output):\n{text}"
        );
        match (state.ended, state.status) {
            (true, Some(code)) => {
                let _ = write!(out, "\n(the program exited with {code})");
            }
            (true, None) => out.push_str("\n(the program ended)"),
            (false, _) if pty::hides_input(&self.control) => out.push_str(wait::ASKING_NOTE),
            (false, _) => {
                let marks = state.screen.marks();
                if let Some(since) = marks.running() {
                    let _ = write!(
                        out,
                        "\n(the shell's command has run for {}s)",
                        since.elapsed().as_secs()
                    );
                } else if let Some(code) = marks.ended().1.and_then(|ended| ended.code) {
                    let _ = write!(out, "\n(the shell's last command exited with {code})");
                }
            }
        }
        out
    }

    /// The folder the program in front is in now (the shell's after a `cd`), if it is known.
    pub fn folder(&self) -> Option<std::path::PathBuf> {
        use std::os::fd::AsRawFd;
        let group = super::super::foreground::group(self.control.as_raw_fd())?;
        super::super::foreground::cwd(group)
    }
}
