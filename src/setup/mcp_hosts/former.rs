//! An agent's server of an older name (LLX Desk 0.1.6's `llx-desk`), taken out once what it runs
//! is gone or is this app: with the agent's own `mcp remove` and that name, or out of OpenCode's
//! JSON, which has no such command. One that runs another program that is there is left alone.

use std::path::Path;

use super::super::former::Swept;
use super::super::shell::{failure, find, login_path, run_with};
use super::{Host, MOST, NAME, opencode_server, shown, write_over};

impl Host {
    /// The command of its server `name` in `text` (its configuration), when it is one to take
    /// out: it runs a program that is gone, or this one (`exe`).
    pub(super) fn stale(&self, text: &str, exe: &Path, name: &str) -> Option<String> {
        let command = self.kept.entry_named(text, name)?;
        gone_or_this(&command, exe).then_some(command)
    }

    /// Its server `name` taken out where [`Self::stale`] finds it so.
    pub(in crate::setup) fn retire(&self, home: &Path, exe: &Path, name: &str) -> Swept {
        let file = self.config(home);
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let at = shown(&file, home);
        if self.stale(&text, exe, name).is_none() {
            return match self.kept.entry_named(&text, name) {
                Some(command) => Swept::Left(format!("its `{name}` in {at} runs {command}")),
                None => Swept::Nothing,
            };
        }
        let gone = format!("its `{name}` server is taken out of {at}");
        let Some(remove) = self.remove else {
            let written = opencode_server(&text, name, None).and_then(|written| {
                write_over(&file, &written).map_err(|why| format!("{at}: {why}"))
            });
            return written.map_or_else(Swept::Failed, |()| Swept::Done(gone));
        };
        let (program, path) = match find(self.program) {
            Ok(found) => found,
            // An agent that is not installed reads nothing; a PATH not read may be read later.
            Err(why) if login_path().is_some() => return Swept::Left(why),
            Err(why) => return Swept::Failed(why),
        };
        let args: Vec<&str> = (remove.iter())
            .map(|arg| if *arg == NAME { name } else { arg })
            .collect();
        match run_with(&program, &args, &path, MOST, None) {
            Ok(ran) if ran.ok => Swept::Done(gone),
            Ok(ran) => Swept::Failed(failure(&ran, &format!("`{}` failed", args.join(" ")))),
            Err(why) => Swept::Failed(why),
        }
    }
}

/// Whether `command` (a program's path, as an agent's entry names it) is gone, or is `exe`.
pub(in crate::setup) fn gone_or_this(command: &str, exe: &Path) -> bool {
    let at = Path::new(command);
    // A link that leads nowhere runs nothing.
    !at.exists() || std::fs::canonicalize(at).is_ok_and(|to| to == exe)
}

#[cfg(test)]
#[path = "former_tests.rs"]
mod tests;
