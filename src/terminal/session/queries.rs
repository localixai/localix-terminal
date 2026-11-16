//! The fixed queries a program asks of its terminal, answered by the session: where the cursor is
//! (`CSI 6 n`) and which terminal it is (XTVERSION, `CSI > q` or `CSI > 0 q`). They are answered
//! whether or not a page shows the terminal, once each, and only so many at a time.

/// A query the session answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Query {
    /// Where the cursor is: answered `CSI row ; column R`.
    Cursor,
    /// The terminal's name and version: answered `DCS > | llxterm <version> ST`.
    Version,
}

/// Each query as a program writes it. None ends another, so a query is found once.
const ASKED: [(&[u8], Query); 3] = [
    (b"\x1b[6n", Query::Cursor),
    (b"\x1b[>q", Query::Version),
    (b"\x1b[>0q", Query::Version),
];

/// The longest of them, less one: what is kept of a read for the next.
const CARRIED: usize = 4;

/// What the terminal calls itself, as `TERM_PROGRAM` names it (one word, read up to the space
/// by most who ask), and its version.
const NAME: &str = concat!("llxterm ", env!("CARGO_PKG_VERSION"));

/// The most answers one read of output is given: a program that asks again and again in one
/// breath is not reading what it is told.
const MOST_REPLIES: usize = 8;

/// The queries in the program's output, read by read: one split across two reads is found whole,
/// the end of one read kept for the start of the next.
#[derive(Default)]
pub(super) struct Queries {
    /// The last bytes of the output so far, fewer than the longest query.
    carried: Vec<u8>,
}

impl Queries {
    /// The queries that end in `chunk`, the output read next, in the order they were asked.
    pub(super) fn asked(&mut self, chunk: &[u8]) -> Vec<Query> {
        let before = self.carried.len();
        self.carried.extend_from_slice(chunk);
        let mut asked = vec![];
        for end in before + 1..=self.carried.len() {
            let written = self.carried.get(..end).unwrap_or_default();
            if let Some((_, query)) = ASKED.iter().find(|(bytes, _)| written.ends_with(bytes)) {
                asked.push(*query);
            }
        }
        let kept = self.carried.len().saturating_sub(CARRIED);
        self.carried.drain(..kept);
        asked
    }
}

/// What the program is told for `asked`, the cursor being at `cursor` (row and column, from 1).
pub(super) fn replies(asked: &[Query], cursor: (u16, u16)) -> Vec<u8> {
    let (row, column) = cursor;
    let mut told = vec![];
    for query in asked.iter().take(MOST_REPLIES) {
        match query {
            Query::Cursor => told.extend(format!("\x1b[{row};{column}R").as_bytes()),
            Query::Version => told.extend(format!("\x1bP>|{NAME}\x1b\\").as_bytes()),
        }
    }
    told
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;
