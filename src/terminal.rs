//! Terminal sessions and their tabs. A [`Session`] is a program in a PTY whose screen is kept as a
//! terminal shows it. A tab's page is xterm.js (vendored, run by its SHA-384): the session paints
//! its raw output into it (`W(...)`), the page sends back the keys, pastes and its size.

mod environment;
mod foreground;
mod keys;
pub mod lineage;
mod pty;
mod screen;
mod session;

pub use session::{DEFAULT_WAIT, Painter, Session, Until};

/// The grid a terminal panel shows: a terminal session is this size.
pub const COLUMNS: u16 = 120;
pub const ROWS: u16 = 40;

/// xterm.js and its stylesheet, vendored (xterm.js 6.0.0): the page runs it by its SHA-384, so a
/// page that was altered never runs a script.
const XTERM: &str = include_str!("terminal/vendor/xterm.js");
const XTERM_SHA384: &str = "f/1U6Z9wM4D71a5eRXEZnyOTMOvjqxr2XLwh+Go1OvIl3L3tOcvUrzudnhbECwl4";
const XTERM_CSS: &str = include_str!("terminal/vendor/xterm.css");

/// The emulator page's own script, and its SHA-384: the page's policy runs it by that hash.
pub(crate) const GLUE: &str = include_str!("terminal/emulator.js");
pub(crate) const GLUE_SHA384: &str =
    "bCXMZDQGoan5Kb6YrBPkOwyOoqtTNR0N5DiUo80jh4xVvRmjUwTgWcWKn5n1Pjy7";

/// The page that shows a session in xterm.js, titled `title`, in the window's light or dark.
/// Its policy runs its own scripts only, by their hashes.
pub fn page(title: &str) -> String {
    let title = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let scripts = format!("'sha384-{XTERM_SHA384}' 'sha384-{GLUE_SHA384}'");
    fill(
        include_str!("terminal/emulator.html"),
        &[
            ("scripts", &scripts),
            ("title", &title),
            ("css", XTERM_CSS),
            ("xterm", XTERM),
            ("glue", GLUE),
        ],
    )
}

/// `template` with each `{{key}}` in it replaced by its value; what a value holds is never read
/// as a key.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut page =
        String::with_capacity(template.len() + values.iter().map(|v| v.1.len()).sum::<usize>());
    let mut rest = template;
    while let Some((before, after)) = rest.split_once("{{") {
        page.push_str(before);
        let (key, next) = after.split_once("}}").unwrap_or((after, ""));
        match values.iter().find(|(known, _)| *known == key) {
            Some((_, value)) => page.push_str(value),
            None => {
                page.push_str("{{");
                page.push_str(key);
            }
        }
        rest = next;
    }
    page.push_str(rest);
    page
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
