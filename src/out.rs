//! The one place the process writes to its standard streams.

use std::io::Write as _;

/// `text` and a line break on standard output: an agent's answer.
pub fn say(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{text}");
}

/// `text` and a line break on standard error: a diagnostic.
pub fn warn(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{text}");
}
