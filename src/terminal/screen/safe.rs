//! What a session reads for itself in a program's output, alongside the terminal core, which gets
//! all of it: the title a program sets (OSC 0 or 2), kept aside as plain words; a shell's marks of
//! its commands (OSC 133), counted; a note for the person (OSC 9, OSC 777), kept as plain text;
//! and what the program says of its state (OSC 7501), kept in its records with its queries
//! counted for the session to answer. The core has no callback for any of these, so each has one
//! source: this reading.

use super::super::marks::Marks;
use super::super::notes::Notes;
use super::super::status::{self, Records, Said};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    Escape,
    /// A CSI sequence, read past up to its final byte.
    Csi,
    /// An OSC, DCS, APC, PM or SOS string, up to BEL or `ESC \`.
    Text,
    TextEscape,
    /// After an escape's intermediate byte (`ESC (`, `ESC #`): up to its final byte.
    Intermediate,
}

/// The longest OSC kept to read a title from; the rest of a longer one is left.
const OSC_MAX: usize = 512;
/// The longest status report (`7501;` and its body): one longer is dropped whole, since a report
/// cut short is a broken one.
const STATUS_MAX: usize = "7501;".len() + status::BODY_MOST;
/// The most characters of a title kept.
const TITLE_MAX: usize = 80;

/// What the session reads of the output; its state carries over between reads.
#[derive(Debug, Default)]
pub(super) struct Safe {
    state: State,
    /// The OSC being read, when the string is one (`ESC ]`), up to `OSC_MAX` bytes (a status
    /// report's `STATUS_MAX`), and whether more came than was kept.
    osc: Option<Vec<u8>>,
    cut: bool,
    pub(super) title: Option<String>,
    /// Counts the times the title changed, so a reader knows when to pass it on.
    pub(super) retitled: u32,
    pub(super) marks: Marks,
    pub(super) notes: Notes,
    pub(super) status: Records,
    /// The status queries not answered yet.
    pub(super) asked: usize,
}

impl Safe {
    /// Reads `bytes`, the output the core is fed next.
    pub(super) fn read(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.step(byte);
        }
    }

    /// One byte of the output, and the state it leaves.
    fn step(&mut self, byte: u8) {
        self.state = match (self.state, byte) {
            (State::Ground, 0x1b) => State::Escape,
            (State::Ground, _) => State::Ground,
            (State::Escape, b'[') => State::Csi,
            (State::Escape, b']') => {
                self.osc = Some(Vec::new());
                self.cut = false;
                State::Text
            }
            // A full reset (RIS) takes what the program said of its state, as it resets the core.
            (State::Escape, b'c') => {
                self.status.reset();
                State::Ground
            }
            (State::Escape, b'P' | b'X' | b'^' | b'_') => State::Text,
            (State::Escape, 0x1b) => State::Escape,
            (State::Escape | State::Intermediate, 0x20..=0x2f) => State::Intermediate,
            (State::Escape | State::Intermediate, _) => State::Ground,
            (State::Csi, 0x1b) => State::Escape,
            (State::Csi, 0x40..=0x7e | 0x18 | 0x1a) => State::Ground,
            (State::Csi, _) => State::Csi,
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
                self.osc_byte(byte);
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
                return self.step(byte);
            }
        };
    }

    /// One byte of a string sequence: kept when it is an OSC with room left, else noted as cut.
    fn osc_byte(&mut self, byte: u8) {
        let Some(osc) = self.osc.as_mut() else { return };
        let most = if osc.starts_with(b"7501;") {
            STATUS_MAX
        } else {
            OSC_MAX
        };
        if osc.len() < most {
            osc.push(byte);
        } else {
            self.cut = true;
        }
    }

    /// A string sequence ended: the title, when it was an OSC that sets one, a shell's mark, a
    /// note for the person, or what the program says of its state.
    fn string_ended(&mut self) {
        let Some(osc) = self.osc.take() else { return };
        let text = String::from_utf8_lossy(&osc);
        if let Some(body) = text.strip_prefix("7501;") {
            match status::read(body).filter(|_| !self.cut) {
                Some(Said::Query) => self.asked += 1,
                Some(Said::Report(report)) => drop(self.status.apply(report)),
                None => {}
            }
            return;
        }
        let Some(("0" | "2", title)) = text.split_once(';') else {
            self.marks.read(&text);
            return self.notes.read(&text);
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
