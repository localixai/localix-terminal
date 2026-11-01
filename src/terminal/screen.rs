//! A terminal session's screen, for a window that shows it: the output applied as a terminal
//! would (cursor moves, erases, redraws), for what the model reads and for a page that needs the
//! whole screen again. The bytes are untrusted, so only text, line moves (`\n`, `\r`, `\t`,
//! `\b`) and CSI sequences reach this emulator: no title, no clipboard write, no string sequence.

pub(super) struct Screen {
    safe: Safe,
    parser: vt100::Parser,
}

impl Screen {
    pub(super) fn new(rows: u16, columns: u16) -> Self {
        Self {
            safe: Safe::default(),
            parser: vt100::Parser::new(rows, columns, 0),
        }
    }

    pub(super) fn feed(&mut self, bytes: &[u8]) {
        let kept = self.safe.keep(bytes);
        self.parser.process(&kept);
    }

    /// Whether the program asked for cursor keys in application mode (DECCKM).
    pub(super) fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    /// The screen as text, its blank rows at the end left out, and where the cursor is (row and
    /// column, from 1).
    pub(super) fn text(&self) -> (String, (u16, u16)) {
        let screen = self.parser.screen();
        let rows: Vec<String> = screen
            .rows(0, screen.size().1)
            .map(|row| row.chars().filter(|c| !c.is_control()).collect::<String>())
            .map(|row| row.trim_end().to_string())
            .collect();
        let used = rows
            .iter()
            .rposition(|row| !row.is_empty())
            .map_or(0, |at| at + 1);
        (rows[..used].join("\n"), self.cursor())
    }

    /// Where the cursor is: row and column, from 1.
    pub(super) fn cursor(&self) -> (u16, u16) {
        let (row, column) = self.parser.screen().cursor_position();
        (row + 1, column + 1)
    }

    /// The screen as a terminal would be told to draw it: a page that reset draws it again.
    pub(super) fn replay(&self) -> Vec<u8> {
        self.parser.screen().contents_formatted()
    }

    /// The grid's rows and columns.
    pub(super) fn size(&self) -> (u16, u16) {
        self.parser.screen().size()
    }

    pub(super) fn resize(&mut self, rows: u16, columns: u16) {
        self.parser.screen_mut().set_size(rows, columns);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    Escape,
    Csi,
    /// An OSC, DCS, APC, PM or SOS string, dropped up to BEL or `ESC \`.
    Text,
    TextEscape,
    /// After `ESC (` and its kin: one more byte, dropped.
    Charset,
}

/// What of the output may reach the emulator; its state carries over between reads.
#[derive(Debug, Default)]
struct Safe {
    state: State,
}

impl Safe {
    fn keep(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut kept = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            self.state = match (self.state, byte) {
                (State::Ground, 0x1b) => State::Escape,
                (State::Ground, b'\n' | b'\r' | b'\t' | 0x08) => {
                    kept.push(byte);
                    State::Ground
                }
                (State::Ground, ..0x20 | 0x7f) => State::Ground,
                (State::Ground, _) => {
                    kept.push(byte);
                    State::Ground
                }
                (State::Escape, b'[') => {
                    kept.extend_from_slice(b"\x1b[");
                    State::Csi
                }
                (State::Escape, b']' | b'P' | b'X' | b'^' | b'_') => State::Text,
                (State::Escape, 0x20..=0x2f) => State::Charset,
                (State::Escape, 0x1b) => State::Escape,
                // Two-byte escapes (save and restore the cursor, a reverse index) are kept;
                // anything else is dropped.
                (State::Escape, b'7' | b'8' | b'M' | b'D' | b'E') => {
                    kept.extend_from_slice(&[0x1b, byte]);
                    State::Ground
                }
                (State::Escape, _) | (State::Charset, _) => State::Ground,
                (State::Csi, 0x20..=0x3f) => {
                    kept.push(byte);
                    State::Csi
                }
                (State::Csi, 0x40..=0x7e) => {
                    kept.push(byte);
                    State::Ground
                }
                // A sequence broken off: what was kept of it is ended harmlessly, as `m`.
                (State::Csi, 0x1b) => {
                    kept.push(b'm');
                    State::Escape
                }
                (State::Csi, _) => {
                    kept.push(b'm');
                    State::Ground
                }
                (State::Text, 0x07) => State::Ground,
                (State::Text, 0x1b) => State::TextEscape,
                (State::Text, _) => State::Text,
                (State::TextEscape, b'\\') => State::Ground,
                (State::TextEscape, _) => State::Text,
            };
        }
        kept
    }
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;
