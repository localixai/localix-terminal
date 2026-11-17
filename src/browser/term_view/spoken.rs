//! What a screen reader is told of a terminal as its screen changes: the new output, read out once,
//! not the echo of the keys the person typed (those it reads as they type), and past twenty lines a
//! sentence in place of the rest, which the person reads line by line instead. The screen of one
//! frame is held up against the last one's: the lines it moved up by are found, and what is in
//! neither is new. The text of the rows a frame did not change is kept from the frames before
//! (`Lines`). Only VoiceOver is read out to yet: on Linux a screen reader is told what the view is,
//! not its text.

use std::collections::HashMap;
#[cfg(any(target_os = "macos", test))]
use std::collections::VecDeque;

use crate::surface::{Frame, RowId};

/// How many lines of new output are read out at once.
#[cfg(any(target_os = "macos", test))]
const LINES: usize = 20;

/// Said in place of the output past `LINES`.
#[cfg(any(target_os = "macos", test))]
pub(super) const MORE: &str = "More output than can be read out: read the terminal's lines.";

/// The screen last heard, and what the person typed whose echo is still to come.
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Default)]
pub(crate) struct Spoken {
    lines: Vec<String>,
    typed: VecDeque<char>,
}

#[cfg(any(target_os = "macos", test))]
impl Spoken {
    /// The person typed `text`: its echo is not read back (they hear their keys as they type).
    pub(crate) fn typed(&mut self, text: &str) {
        self.typed.extend(text.chars().filter(|c| !c.is_control()));
    }

    /// Nothing heard yet: the next screen is a first look, read out not at all.
    #[cfg(target_os = "macos")]
    pub(crate) fn forget(&mut self) {
        self.lines.clear();
        self.typed.clear();
    }

    /// The screen's lines now, top to bottom: what is new in them to read out, if anything.
    pub(crate) fn heard(&mut self, lines: &[String]) -> Option<String> {
        let now: Vec<String> = lines
            .iter()
            .map(|line| line.trim_end().to_string())
            .collect();
        let before = std::mem::replace(&mut self.lines, now);
        // A first look reads nothing out, and the echo of what was typed is in it already.
        if before.len() != self.lines.len() {
            self.typed.clear();
            return None;
        }
        let new = fresh(&before, &self.lines);
        let mut said = String::new();
        // The echo comes in the order typed, among the spaces the program draws around it;
        // anything else means it is not coming, and nothing more is held back for it.
        for c in new.chars() {
            match self.typed.front() {
                Some(&typed) if typed == c => drop(self.typed.pop_front()),
                Some(_) if c.is_whitespace() => said.push(c),
                Some(_) => {
                    self.typed.clear();
                    said.push(c);
                }
                None => said.push(c),
            }
        }
        let lines: Vec<&str> = (said.lines().map(str::trim))
            .filter(|line| !line.is_empty())
            .collect();
        match lines.len() {
            0 => None,
            n if n > LINES => Some(format!("{}\n{MORE}", lines[..LINES].join("\n"))),
            _ => Some(lines.join("\n")),
        }
    }
}

/// The text of each row in view, by its id: a frame carries the text of the rows it changed
/// alone, so what the others say is kept from the frames before.
#[derive(Debug, Default)]
pub(crate) struct Lines(HashMap<RowId, String>);

impl Lines {
    /// Brought up to `frame`: its changed rows read, the rows out of view let go of.
    pub(crate) fn keep(&mut self, frame: &Frame) {
        if frame.full {
            self.0.clear();
        }
        for row in frame.rows.iter().filter(|row| row.dirty) {
            self.0.insert(row.id, row.text.trim_end().to_string());
        }
        self.0
            .retain(|id, _| frame.rows.iter().any(|row| row.id == *id));
    }

    /// The rows of `frame` top to bottom, each its text.
    pub(crate) fn of(&self, frame: &Frame) -> Vec<String> {
        let mut rows: Vec<(u16, &str)> = (frame.rows.iter())
            .map(|row| (row.y, self.0.get(&row.id).map_or("", String::as_str)))
            .collect();
        rows.sort_by_key(|(y, _)| *y);
        rows.into_iter().map(|(_, text)| text.to_string()).collect()
    }
}

/// What `now` holds that `before` did not: the screen moved up by the lines that match best,
/// then each line that is not the one it was (a line grown keeps only what it grew by).
#[cfg(any(target_os = "macos", test))]
fn fresh(before: &[String], now: &[String]) -> String {
    let rows = now.len();
    let matched = |up: usize| {
        (0..rows - up)
            .filter(|&at| !now[at].is_empty() && now[at] == before[at + up])
            .count()
    };
    let up = (0..=rows).max_by_key(|&up| (matched(up), std::cmp::Reverse(up)));
    let up = up.unwrap_or(0);
    let mut parts = vec![];
    for (at, line) in now.iter().enumerate() {
        let was = before.get(at + up).map_or("", String::as_str);
        if line == was {
            continue;
        }
        let grown = line.strip_prefix(was).filter(|_| !was.is_empty());
        parts.push(grown.unwrap_or(line));
    }
    parts.join("\n")
}

#[cfg(test)]
#[path = "spoken_tests.rs"]
mod tests;
