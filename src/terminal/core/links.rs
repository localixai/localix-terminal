//! What a cell of the view links to: the address its program gave its text (OSC 8), else an
//! http or https address written in the line it is part of (the rows that wrap joined, as the
//! program printed it), without the punctuation that ends a sentence; with every stretch of the
//! rows in view the link covers, so a view can underline it. A view asks only as the pointer
//! moves over it with Command held, so a line is read when it is asked for, never ahead.

use ghostty_vt_sys as vt;

use super::Core;
use crate::surface::{Link, Span};

/// How many rows a line is read over, up or down from the one pointed at: an address longer
/// than this many rows is no address a person clicks.
const ROWS_MOST: u64 = 32;

/// Where each address in `line` starts and ends (characters; the end not in): http and https,
/// up to a space or a quote, the punctuation that ends a sentence left off its end.
pub(super) fn addresses(line: &[char]) -> Vec<(usize, usize)> {
    let stops = |c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`');
    let ends = |c: char| matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}');
    let starts = |at: usize, scheme: &str| {
        scheme
            .chars()
            .enumerate()
            .all(|(i, c)| line.get(at + i) == Some(&c))
    };
    let mut found = vec![];
    let mut at = 0;
    while at < line.len() {
        let scheme = match (starts(at, "https://"), starts(at, "http://")) {
            (true, _) => 8,
            (false, true) => 7,
            (false, false) => {
                at += 1;
                continue;
            }
        };
        let host = at + scheme;
        let mut end = host;
        while line.get(end).is_some_and(|c| !stops(*c)) {
            end += 1;
        }
        while end > host && line.get(end - 1).is_some_and(|c| ends(*c)) {
            end -= 1;
        }
        if end >= host + 2 {
            found.push((at, end));
            at = end;
        } else {
            at = host;
        }
    }
    found
}

impl Core {
    /// What the cell at `column` of the row `y` in view links to.
    pub fn link_at(&self, column: u16, y: u16) -> Option<Link> {
        match self.hyperlink_at(y, column) {
            Some(uri) => {
                let columns = self.size().1;
                let same = |at: u16| self.hyperlink_at(y, at).as_deref() == Some(uri.as_str());
                let from = (0..column).rev().take_while(|at| same(*at)).last();
                let to = (column + 1..columns).take_while(|at| same(*at)).last();
                let spans = vec![Span {
                    y,
                    from: from.unwrap_or(column),
                    to: to.unwrap_or(column),
                }];
                Some(Link { uri, spans })
            }
            None => self.address_at(column, y),
        }
    }

    /// The address written in the line the cell at `column` of the row `y` in view is part of.
    fn address_at(&self, column: u16, y: u16) -> Option<Link> {
        let bar: vt::TerminalScrollbar = self.get(vt::TerminalData::SCROLLBAR);
        let total: u64 = self.get::<usize>(vt::TerminalData::TOTAL_ROWS) as u64;
        let row = bar.offset + u64::from(y);
        let mut first = row;
        while first > row.saturating_sub(ROWS_MOST) && first > 0 && self.wraps_on(first - 1) {
            first -= 1;
        }
        let mut last = row;
        while last < row + ROWS_MOST && last + 1 < total && self.wraps_on(last) {
            last += 1;
        }
        let line = self.line_cells(first, last);
        let chars: Vec<char> = line.iter().map(|(c, _)| *c).collect();
        let (from, to) = addresses(&chars).into_iter().find(|(from, to)| {
            (line.get(*from..*to).unwrap_or_default().iter())
                .any(|(c, (at_row, at))| *at_row == row && covers(*c, *at, column))
        })?;
        let uri = chars.get(from..to)?.iter().collect();
        let mut spans: Vec<Span> = vec![];
        for (c, (at_row, at)) in line.get(from..to)? {
            let (Some(view), true) = (
                at_row.checked_sub(bar.offset),
                *at_row < bar.offset + bar.len,
            ) else {
                continue;
            };
            let view = u16::try_from(view).unwrap_or(u16::MAX);
            let end = at + width(*c).saturating_sub(1);
            match spans.last_mut() {
                Some(span) if span.y == view => span.to = end,
                _ => spans.push(Span {
                    y: view,
                    from: *at,
                    to: end,
                }),
            }
        }
        Some(Link { uri, spans })
    }

    /// The rows `first` to `last` of history and screen as one line, each character with its
    /// row and column; a row that wraps keeps its spaces to the edge.
    fn line_cells(&self, first: u64, last: u64) -> Vec<(char, (u64, u16))> {
        let columns = self.size().1;
        let mut line = vec![];
        for row in first..=last {
            let at = u32::try_from(row).unwrap_or(u32::MAX);
            let (Some(start), Some(end)) = (
                self.screen_cell(at, 0),
                self.screen_cell(at, columns.saturating_sub(1)),
            ) else {
                break;
            };
            let one = vt::Selection {
                start,
                end,
                rectangle: false,
            };
            let text = super::text::format(self.plain, &one);
            let mut column = 0u16;
            for c in text.trim_end_matches('\n').chars() {
                line.push((c, (row, column)));
                column = column.saturating_add(width(c));
            }
            if row < last {
                line.extend((column..columns).map(|at| (' ', (row, at))));
            }
        }
        line
    }
}

/// How many cells `c` takes.
fn width(c: char) -> u16 {
    // SAFETY: a pure function of the code point.
    u16::from(unsafe { vt::ghostty_unicode_codepoint_width(u32::from(c)) })
}

/// Whether `c`, at `at`, covers the cell `column`.
fn covers(c: char, at: u16, column: u16) -> bool {
    column >= at && column < at + width(c).max(1)
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
