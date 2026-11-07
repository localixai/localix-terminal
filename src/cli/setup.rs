//! `llx-desk setup`: the integrations (setup.rs) from a terminal, as Settings → Integrations
//! shows them in the window. With nothing, each and its state; with an id, that one put in
//! place (`--off`: taken out); `all`, every one put in place as the app does when it opens;
//! `mcp`, the MCP server's entry for any other host.

use super::{FAILED, OK, UNREACHABLE};
use crate::out::{say, warn};
use crate::setup::{INTEGRATIONS, State, state, turn};

const USAGE: &str = "llx-desk setup [<id> [--off] | all | mcp]";

/// What `setup` was given: whether `--off`, and the names. `--off` goes with one integration
/// only (not `all` or `mcp`, which put in or print), and no other option is known.
fn parse(args: &[String]) -> Option<(bool, Vec<&String>)> {
    let off = args.iter().any(|arg| arg == "--off");
    let named: Vec<&String> = args.iter().filter(|arg| *arg != "--off").collect();
    let one = matches!(named.as_slice(), [id] if !["all", "mcp"].contains(&id.as_str()));
    let known = !named.iter().any(|arg| arg.starts_with('-'));
    (known && (one || !off)).then_some((off, named))
}

pub(super) fn run(args: &[String]) -> u8 {
    let Some((off, named)) = parse(args) else {
        warn(USAGE);
        return UNREACHABLE;
    };
    match named.as_slice() {
        [] => {
            for integration in &INTEGRATIONS {
                let (word, note) = match state(integration.id) {
                    State::On(note) => ("on", note),
                    State::Off(note) => ("off", note),
                    State::Unavailable(note) => ("--", note),
                };
                say(&format!("{word:<3} {:<12} {note}", integration.id));
            }
            say(
                "\n`llx-desk setup <id>` puts one in place, `--off` takes it out for good; `llx-desk setup all` puts every one in place; `llx-desk setup mcp` prints the MCP entry for any other host.",
            );
            OK
        }
        [id] if id.as_str() == "all" => {
            // As the app does when it opens: the command, then every agent's.
            let mut said = crate::setup::opened();
            said.extend(crate::setup::auto());
            if said.is_empty() {
                say("everything is in place");
            }
            for line in said {
                say(&line);
            }
            OK
        }
        [id] if id.as_str() == "mcp" => match crate::setup::mcp_entry() {
            Ok(entry) => {
                say(&entry);
                OK
            }
            Err(why) => {
                warn(&format!("llx-desk setup: {why}"));
                FAILED
            }
        },
        [id] => match turn(id, !off) {
            Ok(done) => {
                say(&done);
                OK
            }
            Err(why) => {
                warn(&format!("llx-desk setup {id}: {why}"));
                FAILED
            }
        },
        _ => {
            warn(USAGE);
            UNREACHABLE
        }
    }
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
