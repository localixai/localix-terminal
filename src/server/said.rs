//! The app's warnings, written where its output goes (`app.log` in the home, or the terminal
//! that started it): what went wrong while serving, each line with its time and where it was
//! said. Without it a warning is said to no one.

use std::io::Write;

/// Writes each warning and error to stderr.
struct Said;

/// The line of a warning said at `stamp` by `target`.
pub(super) fn line(stamp: u64, target: &str, words: &std::fmt::Arguments<'_>) -> String {
    format!("{stamp} {target}: {words}")
}

impl log::Log for Said {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            let said = line(crate::now(), record.target(), record.args());
            let _ = writeln!(std::io::stderr(), "{said}");
        }
    }

    fn flush(&self) {}
}

/// Warnings are written from here on. Said twice, the first stands.
pub(super) fn listen() {
    if log::set_logger(&Said).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

#[cfg(test)]
#[path = "said_tests.rs"]
mod tests;
