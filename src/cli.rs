//! The command line: `llx-desk window`, `serve` and `status`. Exit codes: 0 done, 1 the call
//! failed (the output says why), 3 the daemon could not be reached or the arguments are wrong.

mod call;
mod secret;

use std::process::ExitCode;

use crate::out::{say, warn};

const OK: u8 = 0;
const FAILED: u8 = 1;
const UNREACHABLE: u8 = 3;

const HELP: &str = "llx-desk: a native window of browser pages and terminals that agents work in and a person watches

Usage:
  llx-desk window '<json>' [--as NAME]   one call: open, interact, screenshot, close, show, list, resize
  llx-desk window                        how to call it: the actions and their arguments (JSON Schema)
  llx-desk window --end [--as NAME]      close the owner's tabs
  llx-desk window --said [--as NAME]     print what the person sent from the window since last asked
  llx-desk window --said --follow        keep waiting, and print each comment as it comes (for a monitor)
  llx-desk mcp [--as NAME]               an MCP server on stdio: tools open, act, look, screenshot, tabs, comments
  llx-desk serve [--headless] [--no-window]   the daemon (one per user; started by `window` when needed)
  llx-desk serve status | stop           say what the daemon holds | end it
  llx-desk key < FILE                    save the OpenRouter key `goal` needs (from stdin, kept 0600)
  llx-desk secret NAME < FILE            save a secret (a password) a call types as {{secret:NAME}}, never shown
  llx-desk secret [--forget NAME]        list the secrets saved | forget one
  llx-desk --version

Tabs are kept per owner: the project (the nearest directory with a .git, from CLAUDE_PROJECT_DIR when
set, else the current directory), or NAME with --as.
Give the JSON as the argument, or `-` to read it from stdin.
Environment: OPENROUTER_API_KEY (for `goal`, before the saved key), DESK_DECISION_MODEL, LLX_DESK_HOME (where its log, screenshots and comments go).
Exit codes: 0 done, 1 the call failed, 3 the daemon could not be reached or the arguments are wrong.";

pub fn run(args: Vec<String>) -> ExitCode {
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
        Some("mcp") => crate::mcp::run(&args.collect::<Vec<_>>()),
        Some("serve") => return serve(&args.collect::<Vec<_>>()),
        Some("status") => call::admin("daemon/status", "status"),
        Some("key") => key(),
        Some("secret") => secret::run(&args.collect::<Vec<_>>()),
        Some(other) => {
            warn(&format!("llx-desk: no command `{other}`\n\n{HELP}"));
            UNREACHABLE
        }
    };
    ExitCode::from(code)
}

/// `key`: the OpenRouter key read from stdin and saved for every client, never printed.
fn key() -> u8 {
    let mut key = String::new();
    if let Err(why) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut key) {
        return call::usage(&format!("cannot read the key from stdin: {why}"));
    }
    if key.trim().is_empty() {
        return call::usage("give the key on stdin: llx-desk key < FILE");
    }
    let Some(path) = crate::key::path() else {
        return call::usage("no config directory: set HOME or XDG_CONFIG_HOME");
    };
    match crate::key::save_at(&path, &key) {
        Ok(()) => {
            say(&format!("The key is saved in {}.", path.display()));
            OK
        }
        Err(why) => {
            warn(&format!(
                "llx-desk: cannot save the key in {}: {why}",
                path.display()
            ));
            FAILED
        }
    }
}

/// `serve`, `serve status`, `serve stop`.
fn serve(args: &[String]) -> ExitCode {
    let flags: Vec<&str> = args.iter().map(String::as_str).collect();
    match flags.first().copied() {
        Some("status") => return ExitCode::from(call::admin("daemon/status", "status")),
        Some("stop") => return ExitCode::from(call::admin("daemon/stop", "stop")),
        _ => {}
    }
    if let Some(bad) = flags
        .iter()
        .find(|f| !["--headless", "--no-window"].contains(f))
    {
        return ExitCode::from(call::usage(&format!(
            "serve takes --headless and --no-window, not `{bad}`"
        )));
    }
    let (headless, no_window) = (
        flags.contains(&"--headless"),
        flags.contains(&"--no-window"),
    );
    let bound = match crate::daemon::bind() {
        Ok(crate::daemon::Binding::Bound(bound)) => bound,
        // Another daemon of this user holds the lock, whatever its build: there is one, and the
        // clients reach it. Nothing to say.
        Ok(crate::daemon::Binding::Running) => return ExitCode::SUCCESS,
        Err(why) => return ExitCode::from(call::usage(&format!("cannot serve: {why}"))),
    };
    // A test build never opens a window; `LLX_DESK_NO_WINDOW` asks for the same in any.
    let windowless = no_window
        || cfg!(feature = "test-daemon")
        || std::env::var_os("LLX_DESK_NO_WINDOW").is_some_and(|v| !v.is_empty());
    if windowless || !crate::browser::has_display() {
        return ExitCode::from(
            u8::try_from(crate::daemon::run(&bound, None, false)).unwrap_or(FAILED),
        );
    }
    // The window's event loop owns this thread; the daemon runs beside it, and the process ends
    // with the code it returns.
    let code = crate::browser::host(
        || {},
        move |browser| crate::daemon::run(&bound, Some(browser), headless),
    );
    ExitCode::from(u8::try_from(code).unwrap_or(FAILED))
}
