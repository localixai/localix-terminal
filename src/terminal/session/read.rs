//! What is read of a session: its screen as text, as the model reads it with what is known of
//! the program under it, and the folder it is in.

use std::fmt::Write as _;
use std::sync::Arc;

use super::super::pty;
use super::{Session, wait};

impl Session {
    /// What the program asked to tell the person, with its number, until they type in the
    /// terminal: plain words of the program's, for the person and no agent.
    pub fn note(&self) -> Option<(u32, String)> {
        let mut state = self.shared.lock();
        let (number, words) = state.screen.notes().waiting()?;
        Some((number, words.to_string()))
    }

    /// `heard` each time the note comes or goes.
    pub fn on_note(&self, heard: impl Fn() + Send + Sync + 'static) {
        self.shared.lock().on_note = Some(Arc::new(heard));
    }

    /// The person typed `text` in the window: a note that waited for them is read. What the
    /// window types for them (a reply to the program's question, the mouse, an arrow) starts
    /// with an escape, and reads nothing.
    pub(super) fn attended(&self, text: &str) {
        if text.len() > 1 && text.starts_with('\x1b') {
            return;
        }
        let heard = {
            let mut state = self.shared.lock();
            if !state.screen.notes().heard() {
                return;
            }
            state.on_note.clone()
        };
        if let Some(heard) = heard {
            heard();
        }
    }

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
