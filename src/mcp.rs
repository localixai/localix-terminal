//! `llx-desk mcp`: a stdio MCP server that relays an agent's tool calls to the one daemon
//! over one connection kept for the whole session (`relay`), so there is no process per call and
//! a break between the relay and the daemon is never seen by the host: the stdio side stays up,
//! and calls wait for the connection and go again. The CLI stays as the fallback.

mod catalog;
mod relay;
mod server;
mod tools;
mod upgrade;

use std::io::BufReader;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use relay::{Relay, Timing};
use server::{Out, Server};
use upgrade::{Upgrade, Waiting};

use crate::client::{Client, host_project};
use crate::out::warn;

/// Serve the host on this process's standard streams until its input ends; the exit code.
pub fn run(args: &[String]) -> u8 {
    let mut args = args.iter();
    let (mut named, mut resumed) = (None, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--as" => named = args.next().cloned(),
            "--resumed" => resumed = true,
            other => {
                warn(&format!("llx-desk mcp: no option `{other}`"));
                return 3;
            }
        }
    }
    // The host starts the server in the project; its own variable says so too.
    let cwd = host_project()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let named_by_hand = named.is_some();
    let owner = named.unwrap_or_else(|| crate::desk::project(&cwd).display().to_string());
    let connect = Box::new(|start, notes: &_| {
        Client::connect(start, notes).map_err(|trouble| trouble.to_string())
    });
    let (relay, out) = match (
        Relay::start(&owner, timing(), connect),
        Out::spawn(std::io::stdout()),
    ) {
        (Ok(relay), Ok(out)) => (relay, out),
        (Err(why), _) | (_, Err(why)) => {
            warn(&format!("llx-desk mcp: cannot start ({why})"));
            return 3;
        }
    };
    if resumed {
        out.tools_changed();
    }
    let server = Server {
        relay: relay.clone(),
        out: out.clone(),
        owner: owner.clone(),
        cwd: cwd.display().to_string(),
        running: Arc::new(AtomicUsize::new(0)),
    };
    // Its own descriptor of the input, unbuffered by std, so what waits is what `poll` sees.
    let input = match std::io::stdin().as_fd().try_clone_to_owned() {
        Ok(fd) => std::fs::File::from(fd),
        Err(why) => {
            warn(&format!("llx-desk mcp: cannot read the input ({why})"));
            return 3;
        }
    };
    let every = watch_every();
    let mut upgrade = std::env::current_exe()
        .ok()
        .map(|exe| Upgrade::new(exe, every));
    let again = again_args(named_by_hand.then_some(owner.as_str()));
    let (running, out_now) = (Arc::clone(&server.running), out.clone());
    let idle = move || {
        let Some(upgrade) = upgrade.as_mut() else {
            return;
        };
        if running.load(Ordering::SeqCst) == 0 && upgrade.due() {
            out_now.flush();
            let why = upgrade.take_over(&again);
            warn(&format!(
                "llx-desk mcp: the new build did not start ({why})"
            ));
        }
    };
    server.serve(BufReader::new(Waiting {
        input,
        every,
        idle,
        between: true,
    }));
    relay.close();
    out.flush();
    0
}

/// The arguments a new build is run with: this server's, said resumed.
fn again_args(named: Option<&str>) -> Vec<String> {
    let mut args = vec!["mcp".to_string()];
    if let Some(name) = named {
        args.extend(["--as".to_string(), name.to_string()]);
    }
    args.push("--resumed".into());
    args
}

/// How often the server looks for a new build while idle, and how long one must have settled.
fn watch_every() -> Duration {
    let standard = Duration::from_secs(2);
    if !cfg!(feature = "test-daemon") {
        return standard;
    }
    std::env::var("LLX_DESK_MCP_WATCH_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(standard, Duration::from_millis)
}

/// The relay's times: standard, or in a test build as the test sets them.
fn timing() -> Timing {
    let mut timing = Timing::standard();
    if cfg!(feature = "test-daemon") {
        let millis = |name: &str| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .map(Duration::from_millis)
        };
        timing.ping_every = millis("LLX_DESK_MCP_PING_MS").unwrap_or(timing.ping_every);
        timing.ping_timeout = millis("LLX_DESK_MCP_PING_TIMEOUT_MS").unwrap_or(timing.ping_timeout);
        timing.reconnect_wait = millis("LLX_DESK_MCP_WAIT_MS").unwrap_or(timing.reconnect_wait);
    }
    timing
}
