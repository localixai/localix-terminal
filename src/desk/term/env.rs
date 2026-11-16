//! What a terminal of the app's is told: which terminal it is and of which project, that it reads
//! what a program says of its state, and what the server says every terminal gets (the folder of
//! the app's command, and the variables the programs in it find the app by). A call whose
//! program's lineage is lost (in tmux, or started apart) is known by the terminal its environment
//! names.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::super::Desk;
use crate::terminal::Own;

/// The variables a terminal is given for an environment that the function it is handed reads
/// (the caller's, else the app's): what the server knows of the agents, the desk none of it.
pub(crate) type Agents = fn(&dyn Fn(&str) -> Option<String>) -> Vec<(String, String)>;

/// What every terminal of this run gets, said once by the server as it starts.
struct Told {
    bin: Option<PathBuf>,
    agents: Agents,
}

static TOLD: OnceLock<Told> = OnceLock::new();

/// Which terminal a program runs in (`t3`), and the folder of that terminal's project. Not
/// `LLXTERM_PROJECT`, which names a caller's project whatever its folder: an agent that goes
/// into another repository calls as that one.
const PANE: &str = "LLXTERM_PANE";
const PANE_PROJECT: &str = "LLXTERM_PANE_PROJECT";
/// The terminal reads what a program says of its state (OSC 7501): an agent's integration that
/// would say the same in words of its own leaves it to the terminal.
const PROGRAM_STATUS: &str = "LLXTERM_PROGRAM_STATUS";

impl Desk {
    /// The terminals to come get the app's command from `bin` first on their `PATH`, and what
    /// `agents` gives for their environment. Said once, as the server starts.
    pub fn tell_terminals(&self, bin: Option<PathBuf>, agents: Agents) {
        let _ = TOLD.set(Told { bin, agents });
    }

    /// The terminal the caller's environment `vars` names, while it is open.
    pub fn pane_named(&self, vars: Option<&[(String, String)]>) -> Option<String> {
        let (_, key) = vars?.iter().find(|(name, _)| name == PANE)?;
        let id: u32 = key.strip_prefix('t')?.parse().ok()?;
        crate::lock(&self.sessions)
            .contains_key(&id)
            .then(|| format!("t{id}"))
    }
}

/// What terminal `id` of the project at `folder`, started with `vars` (none: the app's own
/// environment), is told about itself.
pub(super) fn own(id: u32, folder: Option<&Path>, vars: Option<&[(String, String)]>) -> Own {
    let mut own = Own::default();
    if let Some(told) = TOLD.get() {
        let var = |name: &str| match vars {
            Some(vars) => {
                (vars.iter().find(|(set, _)| set == name)).map(|(_, value)| value.clone())
            }
            None => std::env::var(name).ok(),
        };
        own.bin.clone_from(&told.bin);
        own.vars = (told.agents)(&var);
    }
    own.vars.push((PANE.into(), format!("t{id}")));
    own.vars.push((PROGRAM_STATUS.into(), "1".into()));
    if let Some(folder) = folder {
        own.vars
            .push((PANE_PROJECT.into(), folder.display().to_string()));
    }
    own
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
