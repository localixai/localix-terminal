//! A terminal recording read back (asciicast v2, as `LLXTERM_RECORD` writes it): the size it
//! started at, then the program's output and the resizes in order, each with its time.

use std::path::Path;

/// One thing that happened to the terminal.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    /// The program wrote this.
    Output(String),
    /// The terminal became this many rows and columns.
    Resize(u16, u16),
}

/// A recording: its first size (rows, columns) and its events, each with its time in seconds.
pub(crate) struct Cast {
    pub(crate) rows: u16,
    pub(crate) columns: u16,
    pub(crate) events: Vec<(f64, Event)>,
}

impl Cast {
    /// The recording at `path`; what cannot be read panics with where and why.
    pub(crate) fn read(path: &Path) -> Self {
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|why| panic!("{}: {why}", path.display()));
        let mut lines = text.lines();
        let header: serde_json::Value = lines
            .next()
            .and_then(|line| serde_json::from_str(line).ok())
            .unwrap_or_else(|| panic!("{}: no header", path.display()));
        assert_eq!(header["version"], 2, "{}: not asciicast v2", path.display());
        let size = |name: &str| {
            (header[name].as_u64())
                .and_then(|n| u16::try_from(n).ok())
                .unwrap_or_else(|| panic!("{}: no {name}", path.display()))
        };
        let (rows, columns) = (size("height"), size("width"));
        let events = lines
            .enumerate()
            .map(|(at, line)| {
                event(line).unwrap_or_else(|| panic!("{}:{}", path.display(), at + 2))
            })
            .collect();
        Self {
            rows,
            columns,
            events,
        }
    }
}

/// `[seconds, "o", text]` or `[seconds, "r", "COLUMNSxROWS"]`.
fn event(line: &str) -> Option<(f64, Event)> {
    let (at, kind, data): (f64, String, String) = serde_json::from_str(line).ok()?;
    match kind.as_str() {
        "o" => Some((at, Event::Output(data))),
        "r" => {
            let (columns, rows) = data.split_once('x')?;
            Some((at, Event::Resize(rows.parse().ok()?, columns.parse().ok()?)))
        }
        _ => None,
    }
}
