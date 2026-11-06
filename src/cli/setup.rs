//! `llx-desk setup`: the integrations (setup.rs) from a terminal, as Settings → Integrations
//! shows them in the window. With nothing, each and its state; with an id, that one put in
//! place (`--off`: taken out); `mcp`, the MCP server's entry for any other host.

use super::{FAILED, OK, UNREACHABLE};
use crate::out::{say, warn};
use crate::setup::{INTEGRATIONS, State, state, turn};

pub(super) fn run(args: &[String]) -> u8 {
    let off = args.iter().any(|arg| arg == "--off");
    let named: Vec<&String> = args.iter().filter(|arg| *arg != "--off").collect();
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
                "\n`llx-desk setup <id>` puts one in place, `--off` takes it out; `llx-desk setup mcp` prints the MCP entry for any other host.",
            );
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
            warn("llx-desk setup [<id> [--off] | mcp]");
            UNREACHABLE
        }
    }
}
