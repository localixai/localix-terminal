//! Terminal sessions and their tabs. A [`Session`] is a program in a PTY whose screen is kept as a
//! terminal shows it. A tab's page is xterm.js (vendored, run by its SHA-384): the session paints
//! its raw output into it (`W(...)`), the page sends back the keys, pastes and its size.

mod keys;
mod pty;
mod screen;
mod session;
mod view;

pub use session::{DEFAULT_WAIT, Painter, Session, Until};
pub use view::TerminalView;

/// The grid a terminal panel shows: a terminal session is this size.
pub const COLUMNS: u16 = 120;
pub const ROWS: u16 = 40;

/// A colour as red, green and blue.
pub type Rgb = [u8; 3];

/// The person's terminal colours, which the page takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cursor: Rgb,
}

/// xterm.js, its fit addon and its stylesheet, vendored (xterm.js 6.0.0, addon-fit 0.11.0): the
/// page runs them by their SHA-384, so a page that was altered never runs a script.
const XTERM: &str = include_str!("terminal/vendor/xterm.js");
const XTERM_SHA384: &str = "f/1U6Z9wM4D71a5eRXEZnyOTMOvjqxr2XLwh+Go1OvIl3L3tOcvUrzudnhbECwl4";
const FIT: &str = include_str!("terminal/vendor/addon-fit.js");
const FIT_SHA384: &str = "txoiwu4RR2GD3qySbaj+BbzibkLbSJRcfqGYMu6z1EqHil4A2dyBiBW5dlacG6OR";
const XTERM_CSS: &str = include_str!("terminal/vendor/xterm.css");

/// The emulator page's own script, and its SHA-384: the page's policy runs it by that hash.
pub(crate) const GLUE: &str = include_str!("terminal/emulator.js");
pub(crate) const GLUE_SHA384: &str =
    "kGRWX10ozOUtQJZS5KI55hN6v7zvZnpU5wKTvDajWeLzxaSkVYYceosSOSk2TLWI";

/// The page that shows a session in xterm.js, titled `title`, in the person's colours, else the
/// system's. Its policy runs its own scripts only, by their hashes.
pub fn page(title: &str, theme: Option<&Theme>) -> String {
    let colours = theme.map_or(String::new(), |theme| {
        format!(
            ":root{{--fg:{};--bg:{};--cursor:{}}}",
            hex(theme.fg),
            hex(theme.bg),
            hex(theme.cursor)
        )
    });
    let title = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let scripts = format!("'sha384-{XTERM_SHA384}' 'sha384-{FIT_SHA384}' 'sha384-{GLUE_SHA384}'");
    fill(
        include_str!("terminal/emulator.html"),
        &[
            ("scripts", &scripts),
            ("title", &title),
            ("css", XTERM_CSS),
            ("theme", &colours),
            ("xterm", XTERM),
            ("fit", FIT),
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

fn hex([red, green, blue]: Rgb) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
