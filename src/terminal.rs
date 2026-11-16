//! Terminal sessions and their tabs. A [`Session`] is a program in a PTY whose screen is kept as a
//! terminal shows it. A tab's page is xterm.js with its search addon (vendored, each run by its
//! SHA-384): the session paints its raw output into it (`W(...)`), the page sends back the keys,
//! pastes and its size, and finds in what it shows (`findText`, from the window's Find).

mod environment;
mod foreground;
mod hooks;
mod keys;
pub mod lineage;
mod marks;
mod modes;
mod notes;
mod pty;
mod room;
mod screen;
mod session;
mod status;

pub use environment::Own;
pub use hooks::keep_in as keep_shell_files_in;
pub use marks::Ended;
pub use pty::look_for_commands_on;
pub use room::most as room;
pub use session::{DEFAULT_WAIT, Painter, Session, Until};
pub use status::{Asks, Doing, Status};

/// The grid a terminal panel shows: a terminal session is this size.
pub const COLUMNS: u16 = 120;
pub const ROWS: u16 = 40;

/// xterm.js and its stylesheet, vendored (xterm.js 6.0.0): the page runs it by its SHA-384, so a
/// page that was altered never runs a script.
const XTERM: &str = include_str!("terminal/vendor/xterm.js");
const XTERM_SHA384: &str = "f/1U6Z9wM4D71a5eRXEZnyOTMOvjqxr2XLwh+Go1OvIl3L3tOcvUrzudnhbECwl4";
const XTERM_CSS: &str = include_str!("terminal/vendor/xterm.css");
/// xterm.js's search addon, vendored (`@xterm/addon-search` 0.16.0, built with xterm.js 6.0.0):
/// what the page's find bar searches the buffer with and lights the matches by.
const SEARCH: &str = include_str!("terminal/vendor/addon-search.js");
const SEARCH_SHA384: &str = "dj0JnorfKQCQPwELv9I4mHsG6e1V3oaf+DLu0hmYSx6DJ+meQBPtk42UbmpNKOAg";

/// The emulator page's own script after the pages' kit (the terminal, the person's keys, the find
/// row's words and keys, find in the output),
/// and their SHA-384: the page's policy runs it by that hash.
pub(crate) const GLUE: &str = concat!(
    include_str!("page/kit.js"),
    include_str!("terminal/emulator.js"),
    include_str!("terminal/keyboard.js"),
    include_str!("page/find.js"),
    include_str!("terminal/find.js")
);
pub(crate) const GLUE_SHA384: &str =
    "r5ddrI0GOVpKwsJZ+hm2uwaEPjRBnN4vRVnTu0iAstgF8fB7XRjwZ5x4twoGyt1E";

/// The page that shows a session in xterm.js, titled `title`, in the window's light or dark.
/// Its policy runs its own scripts only, by their hashes.
pub fn page(title: &str) -> String {
    let title = crate::page::escaped(title);
    let scripts =
        format!("'sha384-{XTERM_SHA384}' 'sha384-{SEARCH_SHA384}' 'sha384-{GLUE_SHA384}'");
    fill(
        include_str!("terminal/emulator.html"),
        &[
            ("scripts", &scripts),
            ("title", &title),
            ("css", XTERM_CSS),
            ("xterm", XTERM),
            ("search", SEARCH),
            ("glue", GLUE),
            ("find", crate::page::FIND_ROW),
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
