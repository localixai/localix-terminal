//! Find in a terminal's history and screen, case folded for every script (Привет finds привет,
//! Straße finds STRASSE): the core's text read a row at a time, rows that wrap joined into the
//! line a program printed, each character kept with the cell it is in, so a match is known by its
//! cells and lit where it shows. The matches are counted from the top; a find starts at the
//! newest and steps up toward older output. New output is looked in again as a view next draws.

use ghostty_vt_sys as vt;

use super::Core;
use crate::surface::{Count, FindStep, ScrollTo, Span};

/// A cell of the screen and its history: the row from the top of history, and the column.
type Place = (u64, u16);

#[derive(Debug, Default)]
pub(super) struct Found {
    /// Output came since the matches were found.
    pub(super) stale: bool,
    needle: Vec<char>,
    /// The matches, top to bottom: their first cell and their last.
    hits: Vec<(Place, Place)>,
    /// The match shown.
    at: Option<usize>,
}

/// `text` with its case folded, each folded character with the index of the one it came from.
fn folded(text: &[char]) -> Vec<(char, usize)> {
    let mut out = Vec::with_capacity(text.len());
    for (at, c) in text.iter().enumerate() {
        match c {
            'ß' | 'ẞ' => out.extend([('s', at), ('s', at)]),
            'ς' => out.push(('σ', at)),
            _ => out.extend(c.to_lowercase().map(|lower| (lower, at))),
        }
    }
    out
}

impl Core {
    /// A step of a find: the matches of a new needle (the newest shown), the next up or down,
    /// the same counted again, or the find closed; where it is now, the match shown scrolled
    /// into view (but for a recount, which moves nothing).
    pub fn find(&mut self, step: &FindStep) -> Count {
        match step {
            FindStep::Needle(needle) => {
                let chars: Vec<char> = needle.chars().collect();
                self.found.needle = folded(&chars).into_iter().map(|(c, _)| c).collect();
                self.found.at = None;
                self.refind();
                self.found.at = self.found.hits.len().checked_sub(1);
            }
            FindStep::Next | FindStep::Previous => {
                self.refind_if_stale();
                let total = self.found.hits.len();
                let up = *step == FindStep::Next;
                self.found.at = self.found.at.filter(|_| total > 0).map(|at| match up {
                    true => at.checked_sub(1).unwrap_or(total - 1),
                    false => (at + 1) % total,
                });
            }
            FindStep::Again => {
                self.refind_if_stale();
                return self.count();
            }
            FindStep::Close => self.found = Found::default(),
        }
        self.show_found();
        self.count()
    }

    fn count(&self) -> Count {
        Count {
            at: self.found.at.map_or(0, |at| at + 1),
            total: self.found.hits.len(),
        }
    }

    /// The matches in a view `lines` rows tall whose top is the row `top` of history, and the
    /// one shown.
    pub fn found_in_view(&mut self, top: u64, lines: u16) -> (Vec<Span>, Option<Span>) {
        self.refind_if_stale();
        let columns = self.size().1;
        let bottom = top + u64::from(lines);
        let mut spans = vec![];
        let mut shown = None;
        for (index, (from, to)) in self.found.hits.iter().enumerate() {
            for row in from.0.max(top)..=to.0.min(bottom.saturating_sub(1)) {
                let first = if row == from.0 { from.1 } else { 0 };
                let last = if row == to.0 {
                    to.1
                } else {
                    columns.saturating_sub(1)
                };
                let span = Span {
                    y: u16::try_from(row - top).unwrap_or(u16::MAX),
                    from: first,
                    to: last,
                };
                spans.push(span);
                if self.found.at == Some(index) && shown.is_none() {
                    shown = Some(span);
                }
            }
        }
        (spans, shown)
    }

    fn refind_if_stale(&mut self) {
        if self.found.stale && !self.found.needle.is_empty() {
            // The match shown stays the one it was, by its cells: new output adds matches after
            // it. Gone with the history's oldest rows, the one now in its place is shown.
            let shown = (self.found.at).and_then(|at| self.found.hits.get(at).copied());
            self.refind();
            let hits = &self.found.hits;
            self.found.at = shown.and_then(|hit| {
                let at = (hits.iter().position(|other| *other == hit))
                    .unwrap_or_else(|| hits.partition_point(|other| *other < hit));
                at.min(hits.len().checked_sub(1)?).into()
            });
        }
        self.found.stale = false;
    }

    fn refind(&mut self) {
        self.found.stale = false;
        self.found.hits.clear();
        let needle = std::mem::take(&mut self.found.needle);
        if !needle.is_empty() {
            for line in self.lines() {
                hits_in(&line, &needle, &mut self.found.hits);
            }
        }
        self.found.needle = needle;
    }

    /// Scrolls the view so the match shown is in it, near its middle, when it is out of it.
    fn show_found(&mut self) {
        let Some((from, _)) = self
            .found
            .at
            .and_then(|at| self.found.hits.get(at).copied())
        else {
            return;
        };
        let bar: vt::TerminalScrollbar = self.get(vt::TerminalData::SCROLLBAR);
        if from.0 < bar.offset || from.0 >= bar.offset + bar.len {
            self.scroll_to(ScrollTo::Row(from.0.saturating_sub(bar.len / 2)));
        }
    }

    /// The history and the screen as the lines a program printed, each character with its cell.
    fn lines(&self) -> Vec<Vec<(char, Place)>> {
        let total: usize = self.get(vt::TerminalData::TOTAL_ROWS);
        let columns = self.size().1;
        let text = self.screen_text(total);
        let mut lines = vec![];
        let mut line: Vec<(char, Place)> = vec![];
        for (row, words) in (0u64..).zip(text.split('\n')) {
            let mut column = 0u16;
            for c in words.chars() {
                line.push((c, (row, column)));
                // SAFETY: a pure function of the code point.
                let width = unsafe { vt::ghostty_unicode_codepoint_width(u32::from(c)) };
                column = column.saturating_add(u16::from(width));
            }
            if self.wraps_on(row) {
                // The spaces a wrapped row ends with are part of its line.
                line.extend((column..columns).map(|at| (' ', (row, at))));
            } else {
                lines.push(std::mem::take(&mut line));
            }
        }
        lines.push(line);
        lines
    }

    /// Whether the row `row` of history goes on to the next.
    pub(super) fn wraps_on(&self, row: u64) -> bool {
        let Some(at) = self.screen_cell(u32::try_from(row).unwrap_or(u32::MAX), 0) else {
            return false;
        };
        let mut raw: vt::Row = 0;
        // SAFETY: a reference read just now, the terminal unchanged since; `raw` is written.
        let read = unsafe { vt::ghostty_grid_ref_row(&raw const at, &raw mut raw) };
        let mut wraps = false;
        let out: *mut bool = &raw mut wraps;
        // SAFETY: the row's flag is written as a bool.
        let _ = (read == vt::Result::SUCCESS)
            && unsafe { vt::ghostty_row_get(raw, vt::RowData::WRAP, out.cast()) }
                == vt::Result::SUCCESS;
        wraps
    }
}

/// Every match of `needle` in `line`, folded, none overlapping, added to `hits`.
fn hits_in(line: &[(char, Place)], needle: &[char], hits: &mut Vec<(Place, Place)>) {
    let chars: Vec<char> = line.iter().map(|(c, _)| *c).collect();
    let hay = folded(&chars);
    let mut at = 0;
    while at + needle.len() <= hay.len() {
        let found = (hay.get(at..at + needle.len()))
            .is_some_and(|window| window.iter().map(|(c, _)| c).eq(needle.iter()));
        if !found {
            at += 1;
            continue;
        }
        let first = hay.get(at).map(|(_, from)| *from);
        let last = hay.get(at + needle.len() - 1).map(|(_, from)| *from);
        if let (Some((_, from)), Some((c, to))) = (
            first.and_then(|i| line.get(i)),
            last.and_then(|i| line.get(i)),
        ) {
            // SAFETY: a pure function of the code point.
            let width = unsafe { vt::ghostty_unicode_codepoint_width(u32::from(*c)) };
            hits.push((*from, (to.0, to.1 + u16::from(width.max(1)) - 1)));
        }
        at += needle.len();
    }
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
