//! What is read of a session: its screen as text, as the model reads it with what is known of
//! the program under it, and the folder it is in.

use std::fmt::Write as _;
use std::sync::Arc;

use super::super::pty;
use super::{Session, wait};

/// Whether `text`, typed by the person in the window, may put something on a shell's line: a
/// key of theirs, or an arrow that brings a line back from the history; not what the window
/// answers a program for them (its attributes, the pointer, where the cursor is).
pub(super) fn edits_the_line(text: &str) -> bool {
    let arrow = ["\x1b[A", "\x1b[B", "\x1bOA", "\x1bOB"].contains(&text);
    arrow || !(text.len() > 1 && text.starts_with('\x1b'))
}

impl Session {
    /// Whether a line typed now goes to the person's shell at its prompt and to nothing else:
    /// the shell in front (its own process group, not a program it runs), no command at work by
    /// its marks, and nothing typed since it last said its prompt is ready. `marked`: that mark
    /// is needed; else a shell that never marks one (fish, `exec tmux`) takes the line as well.
    pub fn at_prompt(&self, marked: bool) -> bool {
        use std::os::fd::AsRawFd;
        let group = super::super::foreground::group(self.control.as_raw_fd());
        let shell = group.is_some() && group == libc::pid_t::try_from(self.pid()).ok();
        let state = self.shared.lock();
        let marks = state.screen.marks();
        let prompted = marks.prompts() > 0;
        shell
            && !state.ended
            && marks.running().is_none()
            && (prompted || !marked)
            && !(prompted && marks.typed_since_prompt())
    }

    /// What the program asked to tell the person, with its number, until they type in the
    /// terminal: plain words of the program's, for the person and no agent.
    pub fn note(&self) -> Option<(u32, String)> {
        let mut state = self.shared.lock();
        let (number, words) = state.screen.notes().waiting()?;
        Some((number, words.to_string()))
    }

    /// `heard` each time the note comes or goes, or a command of the person's that ended unseen
    /// is seen.
    pub fn on_note(&self, heard: impl Fn() + Send + Sync + 'static) {
        self.shared.lock().told.on_note = Some(Arc::new(heard));
    }

    /// The person typed `text` in the window: a note that waited for them is read, what the
    /// program said it finished or failed is seen, and so is a command of theirs that ended
    /// unseen, and a bell rung unseen. What the window types for them (a reply to the program's question, the mouse, an
    /// arrow) starts with an escape, and reads nothing.
    pub(super) fn attended(&self, text: &str) {
        if text.len() > 1 && text.starts_with('\x1b') {
            return;
        }
        let heard = {
            let mut state = self.shared.lock();
            state.screen.status_mut().seen();
            let rang = state.told.rang.take().is_some();
            let seen = state.told.unseen.take().is_some() || rang;
            if state.screen.notes().heard() || seen {
                state.told.on_note.clone()
            } else {
                None
            }
        };
        if let Some(heard) = heard {
            heard();
        }
        super::status::statused(&self.shared);
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
            (true, status) => {
                out.push('\n');
                out.push_str(&ended_words(status));
            }
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

    /// The command line of the program in front now (past a shell that only runs it, from a
    /// script's own name where a runtime runs it), if it can be read: its arguments are the
    /// program's, untrusted as its output is.
    pub fn front_command(&self) -> Option<Vec<String>> {
        use std::os::fd::AsRawFd;
        let group = super::super::foreground::group(self.control.as_raw_fd())?;
        super::super::foreground::command(group)
    }

    /// The folder the program in front is in now (the shell's after a `cd`), if it is known.
    pub fn folder(&self) -> Option<std::path::PathBuf> {
        use std::os::fd::AsRawFd;
        let group = super::super::foreground::group(self.control.as_raw_fd())?;
        super::super::foreground::cwd(group)
    }
}

/// How a program ended, as its terminal says it: with its exit code when it is known.
pub(super) fn ended_words(status: Option<i32>) -> String {
    match status {
        Some(code) => format!("(the program exited with {code})"),
        None => "(the program ended)".into(),
    }
}
