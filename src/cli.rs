//! The command line, a client of the app: `llx-desk window`, `status`, `quit`, `mcp`, `setup`,
//! `open`, `secret`. Exit codes: 0 done, 1 the call failed (the output says why), 3 the app could
//! not be reached or the arguments are wrong. `serve` is the app's own launch, not the person's.

mod app;
mod call;
mod secret;
mod setup;

use std::process::ExitCode;

use crate::out::{say, warn};

const OK: u8 = 0;
const FAILED: u8 = 1;
const UNREACHABLE: u8 = 3;

const HELP: &str = "llx-desk: a native window of browser pages and terminals that agents work in and a person watches

Usage:
  llx-desk window '<json>' [--as NAME] [--project DIR]   one call: open, interact, screenshot,
                                         close, show, list
  llx-desk window                        how to call it: the actions and their arguments (JSON Schema)
  llx-desk window --end [--as NAME]      close the caller's tabs
  llx-desk window --said [--as NAME]     print what the person sent from the window since last asked
  llx-desk window --board                the project's open cards, for an agent starting work (read
                                         from its file: the app need not run); nothing when none is open
  llx-desk window --said --follow [--json]   keep waiting, and print each comment as it comes (for a
                                         monitor); --json: each one a JSON string on one line
  llx-desk status                        what the app holds (exit 3 when it is not running)
  llx-desk quit                          quit the app: its tabs and terminals end
  llx-desk mcp [--as NAME]               the window tool as an MCP server on stdio, for an agent
                                         whose host speaks MCP (Settings → Integrations adds it)
  llx-desk setup [<id> [--off] | all | mcp]
                                         what the app puts in place by itself: the command, the
                                         Claude Code plugin, the window tool in Codex, Gemini CLI,
                                         Copilot, OpenCode, Hermes; `--off` takes one out for good
  llx-desk open [DIR]                    the app: the window to you (the app's icon does the same);
                                         with DIR, on that folder as a project (`llx-desk open .`)
  llx-desk secret NAME < FILE            save a secret (a password) a call types as {{secret:NAME}}, never shown
  llx-desk secret [--forget NAME]        list the secrets saved | forget one
  llx-desk --version

Tabs, the board and history belong to a project: a folder (the nearest one with a .git around the
current directory; --project DIR or LLX_DESK_PROJECT names another). --as NAME is an agent of it
with tabs of its own.
Give the JSON as the argument, or `-` to read it from stdin.
A call starts the app, out of sight, when it is not running.
Environment: LLX_DESK_PROJECT (the project, when not the one around the current directory); LLX_DESK_HOME (where its log, screenshots and comments go).
Exit codes: 0 done, 1 the call failed, 3 the app could not be reached or the arguments are wrong.";

pub fn run(args: Vec<String>) -> ExitCode {
    // Opened as LLX Desk.app: the app itself, not the CLI's help.
    if crate::bundle::opened(&args) {
        return ExitCode::from(app::run());
    }
    let mut args = args.into_iter();
    let code = match args.next().as_deref() {
        None | Some("help" | "-h" | "--help") => {
            say(HELP);
            OK
        }
        Some("-V" | "--version") => {
            say(concat!("llx-desk ", env!("CARGO_PKG_VERSION")));
            OK
        }
        Some("window") => call::window(&args.collect::<Vec<_>>()),
        Some("serve") => return serve(&args.collect::<Vec<_>>()),
        // Not running is an answer to `status`, but one a script tells apart; nothing to quit is
        // done.
        Some("status") => call::admin("app/status", UNREACHABLE),
        Some("quit") => call::admin("app/quit", OK),
        Some("secret") => secret::run(&args.collect::<Vec<_>>()),
        Some("mcp") => crate::mcp::run(&args.collect::<Vec<_>>()),
        Some("setup") => setup::run(&args.collect::<Vec<_>>()),
        Some("open") => match args.next() {
            Some(dir) => app::open_in(&dir),
            None => app::open(),
        },
        Some(other) => {
            warn(&format!("llx-desk: no command `{other}`\n\n{HELP}"));
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

/// Serve on `bound` until the server stops; its exit code. With a display the window's event
/// loop owns this thread and the server runs beside it; the process ends with the code it
/// returns.
pub(super) fn serving(bound: crate::server::Bound, serve: Serving) -> u8 {
    // A test build never opens a window; `LLX_DESK_NO_WINDOW` asks for the same in any.
    let windowless = serve.windowless
        || cfg!(feature = "test-server")
        || std::env::var_os("LLX_DESK_NO_WINDOW").is_some_and(|v| !v.is_empty());
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
