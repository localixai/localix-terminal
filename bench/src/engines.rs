//! The cores compared, each behind `Engine`: llxterm's own (libghostty-vt behind
//! `llxterm::terminal::Core`), alone and with the frame a view takes after every read, as the app
//! does when it wakes; then the references; and a baseline that takes nothing, for the memory a
//! process needs before any terminal. Every build lists every engine, so the tables are the same;
//! one can be made only in a build that links it (its feature), and each is run in the binary
//! that does (`Home`).

#[cfg(feature = "references")]
mod alacritty;
#[cfg(feature = "ours")]
mod ours;
#[cfg(feature = "rio")]
mod rio;
#[cfg(feature = "references")]
mod vt100;
#[cfg(feature = "references")]
mod wezterm;

use crate::{Engine, Screen};

/// What an engine is in the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// llxterm's own core.
    Ours,
    /// Another terminal's core, the level ours is held to.
    Reference,
    /// Nothing at all: the process alone, measured for memory only.
    Baseline,
}

/// The binary an engine runs in: this crate's `engine`, or `engine-apart` (the `apart/` crate),
/// for a core that cannot be linked beside libghostty-vt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    Bench,
    Apart,
}

impl Home {
    /// The binary's name, beside the one running.
    pub fn binary(self) -> &'static str {
        match self {
            Home::Bench => "engine",
            Home::Apart => "engine-apart",
        }
    }
}

/// How one engine is made: a new terminal, blank, 120 by 40 with 10,000 rows of history.
pub type Make = fn() -> Box<dyn Engine>;

/// One engine: its name in the tables, what it is, the binary it runs in, and how one is made
/// where this build links it.
#[derive(Clone, Copy)]
pub struct Kind {
    pub name: &'static str,
    pub role: Role,
    pub home: Home,
    pub make: Option<Make>,
}

/// The name of llxterm's core alone, whose screen every other engine's is compared with.
pub const OURS: &str = "llxterm";
/// llxterm's core with a frame taken after every read.
pub const FRAMED: &str = "llxterm, framed";
/// The engines the memory benchmark subtracts: each binary's process with no terminal.
pub const NONE: &str = "none";
pub const NONE_APART: &str = "none, apart";

/// `Some` maker of `$engine` where the build has `$feature`, else `None`.
macro_rules! made {
    ($feature:literal, $engine:path) => {{
        #[cfg(feature = $feature)]
        let make: Option<Make> = Some(|| Box::new(<$engine>::new()));
        #[cfg(not(feature = $feature))]
        let make: Option<Make> = None;
        make
    }};
}

/// Every engine, ours first, in the order the tables show them.
pub fn all() -> Vec<Kind> {
    let kind = |name, role, home, make| Kind {
        name,
        role,
        home,
        make,
    };
    let (bench, apart) = (Home::Bench, Home::Apart);
    vec![
        kind(OURS, Role::Ours, bench, made!("ours", ours::Ours)),
        kind(FRAMED, Role::Ours, bench, made!("ours", ours::Framed)),
        kind(
            "alacritty_terminal",
            Role::Reference,
            bench,
            made!("references", alacritty::Alacritty),
        ),
        kind(
            "wezterm-term",
            Role::Reference,
            bench,
            made!("references", wezterm::Wezterm),
        ),
        kind("rio-vt", Role::Reference, apart, made!("rio", rio::Rio)),
        kind(
            "vt100",
            Role::Reference,
            bench,
            made!("references", vt100::Vt100),
        ),
        kind(NONE, Role::Baseline, bench, Some(|| Box::new(Nothing))),
        kind(
            NONE_APART,
            Role::Baseline,
            apart,
            Some(|| Box::new(Nothing)),
        ),
    ]
}

/// The engine called `name`.
pub fn named(name: &str) -> Option<Kind> {
    all().into_iter().find(|kind| kind.name == name)
}

/// The baseline of the binary `home`: what its process holds with no terminal.
pub fn floor_of(home: Home) -> &'static str {
    match home {
        Home::Bench => NONE,
        Home::Apart => NONE_APART,
    }
}

/// No terminal: what is fed is dropped.
struct Nothing;

impl Engine for Nothing {
    fn feed(&mut self, bytes: &[u8]) {
        std::hint::black_box(bytes);
    }

    fn screen(&self) -> Screen {
        Screen::new((0, 0), Vec::new())
    }
}
