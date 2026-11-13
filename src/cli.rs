//! The command line, a client of the app: `llxterm window`, `status`, `quit`, `mcp`, `setup`,
//! `open`, `secret`. Exit codes: 0 done, 1 the call failed (the output says why), 3 the app could
//! not be reached or the arguments are wrong. `serve` is the app's own launch, not the person's.

mod app;
mod call;
mod former;
mod secret;
mod setup;

use std::process::ExitCode;

use crate::out::{say, warn};

const OK: u8 = 0;
const FAILED: u8 = 1;
const UNREACHABLE: u8 = 3;

const HELP: &str = "llxterm: a native window of browser pages and terminals that agents work in and a person watches

Usage:
  llxterm window '<json>' [--as NAME] [--project DIR]   one call: open, interact, screenshot,
                                         close, show, list
  llxterm window '<json>' --peek        as above, a look for a pane: what the person sent stays for your
                                         own next call
  llxterm window                        how to call it: the actions and their arguments (JSON Schema)
  llxterm window --end [--as NAME]      close the caller's tabs
  llxterm window --said [--as NAME]     print what the person sent from the window since last asked
  llxterm window --board                the project's open cards, for an agent starting work (read
                                         from its file: the app need not run); nothing when none is open
  llxterm window --said --follow [--json]   keep waiting, and print each comment as it comes (for a
                                         monitor); --json: each one a JSON string on one line
  llxterm status                        what the app holds (exit 3 when it is not running)
  llxterm quit                          quit the app: its tabs and terminals end
  llxterm mcp [--as NAME]               the window tool as an MCP server on stdio, for an agent
                                         whose host speaks MCP (Settings → Integrations adds it)
  llxterm setup [<id> [--off] | all | mcp]
                                         what the app puts in place by itself: the command, the
                                         Claude Code plugin, the window tool in Codex, Gemini CLI,
                                         Copilot, OpenCode, Hermes; `--off` takes one out for good
  llxterm open [DIR]                    the app: the window to you (the app's icon does the same);
                                         with DIR, on that folder as a project (`llxterm open .`)
  llxterm secret NAME < FILE            save a secret (a password) a call types as {{secret:NAME}}, never shown
  llxterm secret [--forget NAME]        list the secrets saved | forget one
  llxterm --version

Tabs, the board and history belong to a project: a folder (the nearest one with a .git around the
current directory; --project DIR or LLXTERM_PROJECT names another). --as NAME is an agent of it
with tabs of its own.
Give the JSON as the argument, or `-` to read it from stdin.
A call starts the app, out of sight, when it is not running.
Environment: LLXTERM_PROJECT (the project, when not the one around the current directory); LLXTERM_HOME (where its log, screenshots and comments go).
Exit codes: 0 done, 1 the call failed, 3 the app could not be reached or the arguments are wrong.";

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
    let code = match command.as_deref() {
        None | Some("help" | "-h" | "--help") => {
            say(HELP);
            OK
        }
        Some("-V" | "--version") => {
            say(concat!("llxterm ", env!("CARGO_PKG_VERSION")));
            OK
        }
        Some("window") => call::window(&rest),
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
        Some("setup") => setup::run(&rest),
        Some("open") => match rest.as_slice() {
            [] => app::open(),
            [help] if matches!(help.as_str(), "-h" | "--help") => {
                say(HELP);
                OK
            }
            [dir] => app::open_in(dir),
            _ => call::usage("open takes one folder at most"),
        },
        Some(other) => {
            warn(&format!("llxterm: no command `{other}`\n\n{HELP}"));
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
    // The server a call started looks after the agents as the app opened does. A build for
    // development does not: what it would write names a program that is gone with the next build.
    if crate::bundle::in_place() && !cfg!(debug_assertions) {
        app::keep_well();
    }
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

/// The kinds of tab that draw themselves, listed once before the window and the desk are made:
/// both read the list, and neither names a kind.
fn list_kinds() {
    let mut kinds: Vec<std::sync::Arc<dyn crate::kind::Kind>> = Vec::new();
    match crate::catalog::kind::Store::open(&crate::paths::data()) {
        Ok(store) => kinds.push(store),
        Err(why) => log::warn!("there is no Store in this run: {why}"),
    }
    crate::kind::install(kinds);
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
