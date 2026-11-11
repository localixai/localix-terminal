//! A terminal session's screen, for a window that shows it: the output applied as a terminal
//! would (cursor moves, erases, redraws), for what the model reads and for a page that needs the
//! whole screen again. The bytes are untrusted, so only text, line moves (`\n`, `\r`, `\t`,
//! `\b`) and CSI sequences reach this emulator: no title, no clipboard write, no string sequence.
//! The title a program sets (OSC 0 or 2) is kept aside as plain text, for the tab's subtitle,
//! and a shell's marks of its commands (OSC 133) are counted.

use super::marks::Marks;

/// What puts right a terminal a command left wrong: no mouse reports of any kind, in any
/// encoding, and the cursor shown.
const RIGHTED: &[u8] = b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?25h";

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

    /// Takes in the program's output. What it gives back is the terminal's own, to be drawn
    /// after it: when the shell says a command ended and runs nothing more, what that command
    /// left on (the mouse reported, the cursor hidden) is put right, or the prompt would take
    /// clicks as typing.
    pub(super) fn feed(&mut self, bytes: &[u8]) -> &'static [u8] {
        let ended = self.safe.marks.ended().0;
        let kept = self.safe.keep(bytes);
        self.parser.process(&kept);
        let screen = self.parser.screen();
        let left_on =
            screen.hide_cursor() || screen.mouse_protocol_mode() != vt100::MouseProtocolMode::None;
        let marks = &self.safe.marks;
        if marks.ended().0 == ended || marks.running().is_some() || !left_on {
            return &[];
        }
        self.parser.process(RIGHTED);
        RIGHTED
    }

    /// The title the program set last, cleaned to plain words; `None` until it sets one.
    pub(super) fn title(&self) -> Option<&str> {
        self.safe.title.as_deref()
    }

    /// Counts the times the title changed.
    pub(super) fn retitled(&self) -> u32 {
        self.safe.retitled
    }

    /// Whether the program asked for cursor keys in application mode (DECCKM).
    pub(super) fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    /// What the shell said of its commands.
    pub(super) fn marks(&self) -> &Marks {
        &self.safe.marks
    }

    /// Whether the program asked to be told where a paste starts and ends.
    pub(super) fn bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste()
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

    /// The screen as text to look in: a row that wraps on to the next is joined to it with no line
    /// break and its trailing spaces kept, so a text that crosses the pane's edge is found whole.
    /// The rows are as `text` has them otherwise.
    pub(super) fn search_text(&self) -> String {
        let screen = self.parser.screen();
        let rows = screen.size().0;
        let mut out = String::new();
        for (at, row) in (0..rows).zip(screen.rows(0, screen.size().1)) {
            let row: String = row.chars().filter(|c| !c.is_control()).collect();
            if screen.row_wrapped(at) {
                out.push_str(&row);
            } else {
                out.push_str(row.trim_end());
                out.push('\n');
            }
        }
        out.truncate(out.trim_end_matches('\n').len());
        out
    }

    /// Where the cursor is: row and column, from 1.
    pub(super) fn cursor(&self) -> (u16, u16) {
        let (row, column) = self.parser.screen().cursor_position();
        (row + 1, column + 1)
    }

    /// The screen as a terminal would be told to draw it: a page that reset draws it again, in the
    /// alternate screen if the program is in it, and with the input modes it asked for (bracketed
    /// paste, the mouse, application cursor keys), which a reset page has lost.
    pub(super) fn replay(&self) -> Vec<u8> {
        let screen = self.parser.screen();
        let mut drawn = if screen.alternate_screen() {
            b"\x1b[?1049h".to_vec()
        } else {
            Vec::new()
        };
        drawn.extend(screen.state_formatted());
        drawn
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

/// The longest OSC kept to read a title from; the rest of a longer one is dropped.
const OSC_MAX: usize = 512;
/// The most characters of a title kept.
const TITLE_MAX: usize = 80;

/// What of the output may reach the emulator; its state carries over between reads.
#[derive(Debug, Default)]
struct Safe {
    state: State,
    /// The OSC being read, when the string is one (`ESC ]`), up to `OSC_MAX` bytes.
    osc: Option<Vec<u8>>,
    title: Option<String>,
    /// Counts the times the title changed, so a reader knows when to pass it on.
    retitled: u32,
    marks: Marks,
}

impl Safe {
    fn keep(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut kept = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            self.step(byte, &mut kept);
        }
        kept
    }

    /// One byte of the output: what of it is kept, and the state it leaves.
    fn step(&mut self, byte: u8, kept: &mut Vec<u8>) {
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
            (State::Escape, b']') => {
                self.osc = Some(Vec::new());
                State::Text
            }
            (State::Escape, b'P' | b'X' | b'^' | b'_') => State::Text,
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
            (State::Text, 0x07) => {
                self.string_ended();
                State::Ground
            }
            // CAN and SUB cancel a string, as they do any sequence.
            (State::Text, 0x18 | 0x1a) => {
                self.osc = None;
                State::Ground
            }
            (State::Text, 0x1b) => State::TextEscape,
            (State::Text, _) => {
                if let Some(osc) = self.osc.as_mut().filter(|osc| osc.len() < OSC_MAX) {
                    osc.push(byte);
                }
                State::Text
            }
            (State::TextEscape, b'\\') => {
                self.string_ended();
                State::Ground
            }
            // A string broken off by another escape: dropped, and that escape begins a
            // sequence of its own, as a terminal reads it.
            (State::TextEscape, _) => {
                self.osc = None;
                self.state = State::Escape;
                return self.step(byte, kept);
            }
        };
    }

    /// A string sequence ended: the title, when it was an OSC that sets one, or a shell's mark.
    fn string_ended(&mut self) {
        let Some(osc) = self.osc.take() else { return };
        let text = String::from_utf8_lossy(&osc);
        let Some(("0" | "2", title)) = text.split_once(';') else {
            return self.marks.read(&text);
        };
        if let Some(title) = clean_title(title)
            && self.title.as_ref() != Some(&title)
        {
            self.title = Some(title);
            self.retitled = self.retitled.wrapping_add(1);
        }
    }
}

/// A title as words: control characters out, and the spinner or status mark a program puts in
/// front (`✳`, `⠋`, `·`) left off, so a title that only spins reads the same. `None` when no
/// words are left.
fn clean_title(title: &str) -> Option<String> {
    let words: String = title
        .chars()
        .filter(|c| !c.is_control())
        .skip_while(|c| !c.is_alphanumeric())
        .take(TITLE_MAX)
        .collect();
    let words = words.trim_end();
    (!words.is_empty()).then(|| words.to_string())
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;
