//! Terminal sessions and their tabs. A [`Session`] is a program in a PTY whose screen is kept by
//! the terminal core as a terminal shows it, for agents to read and wait on, and drawn by the
//! window's native terminal view through `surface::Surface`.

mod core;
mod environment;
mod foreground;
mod hooks;
mod keys;
pub mod lineage;
mod marks;
mod notes;
mod pty;
mod room;
mod screen;
mod session;
mod status;

pub use core::{Content, Core, Refused, Render, Step};
pub use environment::Own;
pub use hooks::keep_in as keep_shell_files_in;
pub use marks::Ended;
pub use pty::look_for_commands_on;
pub use room::most as room;
pub use session::{DEFAULT_WAIT, Session, Until, View};
pub use status::{Asks, Doing, Status};

/// The grid a terminal panel shows: a terminal session is this size.
pub const COLUMNS: u16 = 120;
pub const ROWS: u16 = 40;

/// The terminal type a terminal's programs are told they run in (`TERM`), which the core also
/// answers a program's query of its terminfo name with (XTGETTCAP `TN`), and a recording says.
const TERM: &str = "xterm-256color";
