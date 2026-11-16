//! The command line, a client of the app: `llxterm window`, the short commands (`click e12 + wait
//! Saved`, verbs.rs), `guide [topic]`, `status`, `quit`, `mcp`, `open`, `secret`. Exit codes: 0 done, 1 the call failed (the output says why), 3 the app could
//! not be reached or the arguments are wrong. `serve` is the app's own launch, not the person's.

mod app;
mod call;
mod former;
mod secret;
mod verbs;

use std::process::ExitCode;

use crate::out::{say, warn};

const OK: u8 = 0;
const FAILED: u8 = 1;
const UNREACHABLE: u8 = 3;

const HELP: &str = "llxterm: the command of Localix Terminal, the terminal for you and your coding agents

Usage:
  llxterm window '<json>' [--as NAME] [--project DIR]   one call: open, interact, screenshot,
                                         close, show, list, steps, guide
  llxterm window '<json>' --peek        as above, a look for a pane: what the person sent stays for your
                                         own next call
  llxterm window                        how to call it: the actions and their arguments (JSON Schema)
  llxterm guide [TOPIC]                 how to use the window tool, a topic at a time: with none,
                                         the overview and the list of topics
  llxterm window --end [--as NAME]      close the caller's tabs
  llxterm window --said [--as NAME]     print what the person sent from the window since last asked
  llxterm window --board                the project's open cards, for an agent starting work (read
                                         from its file: the app need not run); nothing when none is open
  llxterm window --said --follow [--json]   keep waiting, and print each comment as it comes (for a
                                         monitor); --json: each one a JSON string on one line
  llxterm window --watch                the app, this project's tabs and board, and the person's
                                         comments: a JSON line for each change, as it comes

Short commands, each one action of `window`; ` + ` joins several into one call, and a step that
misses (a ref gone, text that never showed) stops the rest and exits 1 (`llxterm guide commands`):
";

const HELP_ADMIN: &str = "  llxterm status                        what the app holds (exit 3 when it is not running)
  llxterm quit                          quit the app: its tabs and terminals end
  llxterm mcp [--as NAME]               the window tool as an MCP server on stdio, for an agent
                                         in a terminal of the app's whose host speaks MCP
  llxterm open [DIR]                    the app: the window to you (the app's icon does the same);
                                         with DIR, a folder, on it as a project (`llxterm open .`)
  llxterm secret NAME < FILE            save a secret (a password) a call types as {{secret:NAME}}, never shown
  llxterm secret [--forget NAME]        list the secrets saved | forget one
  llxterm --version

Tabs, the board and history belong to a project: a folder (the nearest one with a .git around the
current directory; --project DIR or LLXTERM_PROJECT names another). --as NAME is an agent of it
with tabs of its own.
Give the JSON as the argument, or `-` to read it from stdin.
A call starts the app, out of sight, when it is not running.
In a terminal of the app's, `llxterm` is on the PATH, LLXTERM_PANE names the terminal (t3) and
LLXTERM_PANE_PROJECT its project's folder, which a call from a folder in it with no repository of its own takes.
Environment: LLXTERM_PROJECT (the project, when not the one around the current directory); LLXTERM_HOME (where its log, screenshots and comments go).
Exit codes: 0 done, 1 the call failed (or a short command's step missed), 3 the app could not be
reached or the arguments are wrong.";

/// The help: the calls, the short commands from their table, then the rest.
fn help() -> String {
    format!("{HELP}{}\n{HELP_ADMIN}", crate::short::help())
}

pub fn run(args: Vec<String>) -> ExitCode {
    let opened = crate::bundle::opened(&args);
    // What LLX Desk 0.1.6 left, put right before anything reads the app's files.
    if let Some(code) = former::begin(&args, opened) {
        return ExitCode::from(code);
    }
    // Opened as Localix Terminal.app: the app itself, not the CLI's help.
    if opened {
        return ExitCode::from(app::run());
    }
    let mut args = args.into_iter();
    let command = args.next();
    let rest: Vec<String> = args.collect();
    let whole: Vec<String> = command.iter().chain(&rest).cloned().collect();
    let code = match command.as_deref() {
        None | Some("help" | "-h" | "--help") => {
            say(&help());
            OK
        }
        Some("-V" | "--version") => {
            say(concat!("llxterm ", env!("CARGO_PKG_VERSION")));
            OK
        }
        Some("window") => call::window(&rest),
        // The guide as this build has it, a topic at a time; it starts nothing.
        Some("guide") => match rest.as_slice() {
            [] => guide(None),
            [topic] => guide(Some(topic)),
            _ => call::usage("guide takes one topic at most"),
        },
        Some("serve") => return serve(&rest),
        Some(name @ ("status" | "quit")) if !rest.is_empty() => {
            call::usage(&format!("{name} takes no arguments"))
        }
        // Not running is an answer to `status`, but one a script tells apart; nothing to quit is
        // done.
        Some("status") => call::admin("app/status", UNREACHABLE),
        Some("quit") => call::admin("app/quit", OK),
        Some("secret") => secret::run(&rest),
        Some("mcp") => crate::mcp::run(&rest),
        // With nothing or a folder, the app; anything else is a page (verbs.rs).
        Some("open") => match rest.as_slice() {
            [] => app::open(),
            [flag] if matches!(flag.as_str(), "-h" | "--help") => {
                say(&help());
                OK
            }
            [dir] if std::path::Path::new(dir).is_dir() => app::open_in(dir),
            _ => verbs::run(&whole),
        },
        Some(verb) if crate::short::is_verb(verb) || matches!(verb, "--as" | "--project") => {
            verbs::run(&whole)
        }
        Some(other) => {
            let why = crate::short::steps(&whole, &mut || Err(String::new())).err();
            let why = why.unwrap_or_else(|| format!("no command `{other}`"));
            warn(&format!("llxterm: {why}\n\n{}", help()));
            UNREACHABLE
        }
    };
    ExitCode::from(code)
}

/// `serve`: the app's launch out of sight, as a call starts it (LaunchServices, or a plain
/// process where there is no bundle).
fn serve(args: &[String]) -> ExitCode {
    let flags: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Some(bad) = flags
        .iter()
        .find(|f| !["--headless", "--no-window"].contains(f))
    {
        return ExitCode::from(call::usage(&format!(
            "serve takes --headless and --no-window, not `{bad}`"
        )));
    }
    crate::server::log_to_home();
    let bound = match crate::server::bind() {
        Ok(crate::server::Binding::Bound(bound)) => bound,
        // Another server of this user holds the lock, whatever its build: there is one, and the
        // clients reach it. Nothing to say.
        Ok(crate::server::Binding::Running) => return ExitCode::SUCCESS,
        Err(why) => return ExitCode::from(call::usage(&format!("cannot serve: {why}"))),
    };
    let serve = Serving {
        headless: flags.contains(&"--headless"),
        windowless: flags.contains(&"--no-window"),
        opened: false,
    };
    ExitCode::from(serving(bound, serve))
}

/// How the server runs.
#[derive(Clone, Copy)]
pub(super) struct Serving {
    /// Its pages drawn out of sight.
    pub headless: bool,
    /// No window at all: terminals only.
    pub windowless: bool,
    /// The person opened the app: the window comes to them at once.
    pub opened: bool,
}

/// A kind of tab that draws itself, as the app lists it: what it says of itself, read without
/// opening it, and how the app opens it from the data folder.
struct Listed {
    about: &'static crate::kind::About,
    open: fn(&std::path::Path) -> Result<std::sync::Arc<dyn crate::kind::Kind>, String>,
}

/// The kinds of tab that draw themselves, listed once: the app opens them before the window and
/// the desk are made, which both read the list and name no kind; the guide reads what each says.
static KINDS: [Listed; 1] = [Listed {
    about: &crate::catalog::kind::ABOUT,
    open: |data| Ok(crate::catalog::kind::Store::open(data)?),
}];

fn list_kinds() {
    let data = crate::paths::data();
    let kinds = (KINDS.iter())
        .filter_map(|listed| {
            (listed.open)(&data)
                .inspect_err(|why| {
                    log::warn!("there is no {} in this run: {why}", listed.about.title)
                })
                .ok()
        })
        .collect();
    crate::kind::install(kinds);
}

/// What each listed kind says of itself, none opened: the guide's and the description's.
fn kinds() -> Vec<&'static crate::kind::About> {
    KINDS.iter().map(|listed| listed.about).collect()
}

/// `guide [topic]`: the guide as this build has it, the kinds it lists among the topics.
fn guide(topic: Option<&str>) -> u8 {
    match crate::desk::guide(topic, &kinds()) {
        Ok(text) => {
            say(&text);
            OK
        }
        Err(why) => call::usage(&why),
    }
}

/// Serve on `bound` until the server stops; its exit code. With a display the window's event
/// loop owns this thread and the server runs beside it; the process ends with the code it
/// returns.
pub(super) fn serving(bound: crate::server::Bound, serve: Serving) -> u8 {
    // A test build never opens a window; `LLXTERM_NO_WINDOW` asks for the same in any.
    let windowless = serve.windowless
        || cfg!(feature = "test-server")
        || std::env::var_os("LLXTERM_NO_WINDOW").is_some_and(|v| !v.is_empty());
    list_kinds();
    let begin = crate::server::Begin {
        headless: serve.headless,
        opened: serve.opened,
    };
    if windowless || !crate::browser::has_display() {
        return u8::try_from(crate::server::run(&bound, None, begin)).unwrap_or(FAILED);
    }
    let code =
        crate::browser::host(move |browser| crate::server::run(&bound, Some(browser), begin));
    u8::try_from(code).unwrap_or(FAILED)
}
