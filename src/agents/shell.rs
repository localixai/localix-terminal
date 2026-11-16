//! Programs of the person's, run as their terminal would: found on their login shell's PATH (an
//! app started from the Finder has only the system's), each within a deadline and stopped past
//! it; and the variables of that shell that say where their agents keep things.

use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What a program said and how it ended.
pub(crate) struct Ran {
    pub ok: bool,
    pub out: String,
}

/// How long a PATH read is kept: the person installs a version manager or edits their profile
/// while the app runs. A read that failed is not tried again for a shorter while.
const KEPT: Duration = Duration::from_secs(300);
const KEPT_FAILED: Duration = Duration::from_secs(60);

/// What the PATH is printed between, so what an rc file or the shell itself prints is not taken for it.
const MARK: &str = "__LLX_PATH__";

/// How long the login shell has to say its PATH.
const ASK_WITHIN: Duration = Duration::from_secs(10);

/// How long a caller waits for another's question while it has a PATH read before to go on with.
const STALE_WAIT: Duration = Duration::from_secs(1);

/// What each of the agents' own variables is printed between, as `NAME=value`.
const VAR_MARK: &str = "__LLX_VAR__";

/// The variables of the login shell kept beside its PATH: where the person's agents keep their
/// configuration when they moved it from its place under the home directory. An agent's own
/// command is run with them, so it writes where the agent in their terminal reads.
const HOMES: [&str; 5] = [
    "CLAUDE_CONFIG_DIR",
    "CODEX_HOME",
    "GEMINI_CLI_HOME",
    "COPILOT_HOME",
    "XDG_CONFIG_HOME",
];

/// What the login shell's reads have come to.
struct Known {
    /// The last read: when it was made, and the PATH it said (none when the shell did not).
    read: Option<(Instant, Option<String>)>,
    /// The agents' variables the shell had set, when it last said.
    vars: Vec<(String, String)>,
    /// A caller is asking the shell now.
    asking: bool,
}

impl Known {
    fn fresh(&self) -> bool {
        (self.read.as_ref()).is_some_and(|(at, path)| {
            at.elapsed() < if path.is_some() { KEPT } else { KEPT_FAILED }
        })
    }

    fn path(&self) -> Option<String> {
        self.read.as_ref().and_then(|(_, path)| path.clone())
    }
}

static KNOWN: std::sync::Mutex<Known> = std::sync::Mutex::new(Known {
    read: None,
    vars: vec![],
    asking: false,
});
static ANSWERED: std::sync::Condvar = std::sync::Condvar::new();

/// The PATH a new terminal of the person's has: their login shell's. None when the shell does
/// not answer within a few seconds. It asks the shell, which may take seconds: never on the
/// window's thread.
pub(crate) fn login_path() -> Option<String> {
    // A login shell may take seconds to start (a version manager's setup), and several callers
    // may ask at once: one asks, the others wait for its answer (the PATH read before, when there
    // is one, once they have waited a moment). The lock is held for the bookkeeping only, never
    // while the shell runs.
    let mut known = crate::lock(&KNOWN);
    let began = Instant::now();
    loop {
        if known.fresh() {
            return known.path();
        }
        if !known.asking {
            known.asking = true;
            break;
        }
        let most = if known.path().is_some() {
            STALE_WAIT
        } else {
            ASK_WITHIN
        };
        let left = most.saturating_sub(began.elapsed());
        if left.is_zero() {
            return known.path();
        }
        known = crate::wait_timeout(&ANSWERED, known, left);
    }
    drop(known);
    let asked = ask_login_shell();
    let mut known = crate::lock(&KNOWN);
    // A read that fails keeps what was read before.
    let before = known.path();
    let path = match asked {
        Some((path, vars)) => {
            known.vars = vars;
            Some(path)
        }
        None => before,
    };
    known.read = Some((Instant::now(), path));
    known.asking = false;
    ANSWERED.notify_all();
    known.path()
}

/// The variables of the person's login shell that say where their agents keep things
/// (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`…), each when it is set: the shell's, else this process's
/// own (the app started from a terminal). It may ask the shell: never on the window's thread.
pub(crate) fn login_vars() -> BTreeMap<String, String> {
    let _ = login_path();
    let said = crate::lock(&KNOWN).vars.clone();
    (HOMES.iter())
        .filter_map(|name| {
            let value = (said.iter().find(|(known, _)| known == name))
                .map(|(_, value)| value.clone())
                .or_else(|| std::env::var(name).ok())
                .filter(|value| !value.is_empty())?;
            Some((name.to_string(), value))
        })
        .collect()
}

fn ask_login_shell() -> Option<(String, Vec<(String, String)>)> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut asked = Command::new(&shell);
    // `printf` goes through its format once for each pair it is given: a line for each variable.
    let pairs: Vec<String> = (HOMES.iter())
        .map(|name| format!("{name} \"${name}\""))
        .collect();
    asked.args([
        "-l",
        "-i",
        "-c",
        &format!(
            "printf '\\n{MARK}%s{MARK}\\n' \"$PATH\"; printf '{VAR_MARK}%s=%s{VAR_MARK}\\n' {}",
            pairs.join(" ")
        ),
    ]);
    let ran = within(asked, ASK_WITHIN).ok()?;
    let path = ran.ok.then(|| path_in(&ran.out)).flatten()?;
    Some((path, vars_in(&ran.out)))
}

/// The agents' variables a shell said it has set, each on a line of its own between its marks.
fn vars_in(said: &str) -> Vec<(String, String)> {
    (said.lines())
        .filter_map(|line| {
            let pair = line.trim().strip_prefix(VAR_MARK)?.strip_suffix(VAR_MARK)?;
            let (name, value) = pair.split_once('=')?;
            (HOMES.contains(&name) && !value.is_empty())
                .then(|| (name.to_string(), value.to_string()))
        })
        .collect()
}

/// The text between the last pair of marks in what a shell said.
fn path_in(said: &str) -> Option<String> {
    let (before, _) = said.rsplit_once(MARK)?;
    let (_, path) = before.rsplit_once(MARK)?;
    Some(path.to_string()).filter(|path| !path.is_empty())
}

/// The program `name` on `path` (a login shell's PATH), without starting anything.
pub(crate) fn which(name: &str, path: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|program| program.is_file())
}

/// Why a program that failed did: what it said, or when it said nothing, `what` it means.
pub(crate) fn failure(ran: &Ran, what: &str) -> String {
    match ran.out.trim() {
        "" => what.to_string(),
        said => said.to_string(),
    }
}

/// `program` run with `args`, its PATH `path` and the variables `vars`, stopped past `most`:
/// what it said (its output, then its errors) and whether it ended well.
pub(crate) fn run(
    program: &Path,
    args: &[&str],
    path: &str,
    vars: &BTreeMap<String, String>,
    most: Duration,
) -> Result<Ran, String> {
    let mut command = Command::new(program);
    command.args(args).env("PATH", path).envs(vars);
    within(command, most).map_err(|why| format!("{} {} {why}", program.display(), args.join(" ")))
}

/// A file of this run's own, made new (it never follows a link or takes over one that was there)
/// and for its owner only, under a name no other user can guess: the shared temporary directory
/// is another user's to write in too.
fn private_file() -> Result<(PathBuf, std::fs::File), String> {
    let path = std::env::temp_dir().join(format!("llxterm-run-{}", crate::token()));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|why| format!("could not make its temporary file ({why})"))?;
    Ok((path, file))
}

/// What `command` says, when it ends within `most`; one that does not is stopped. Its output
/// goes to a file, not a pipe: an interactive shell leaves helpers behind (a prompt's git
/// watcher) that would hold a pipe open long after the shell ended.
fn within(mut command: Command, most: Duration) -> Result<Ran, String> {
    let (path, out) = private_file()?;
    let ran = within_file(&mut command, most, out);
    let _ = std::fs::remove_file(path);
    ran
}

fn within_file(command: &mut Command, most: Duration, out: std::fs::File) -> Result<Ran, String> {
    let err = out
        .try_clone()
        .map_err(|why| format!("could not make its temporary file ({why})"))?;
    let mut said = out
        .try_clone()
        .map_err(|why| format!("could not read its output ({why})"))?;
    crate::fds::as_started(command);
    let mut child = (command.stdin(Stdio::null()))
        .stdout(out)
        .stderr(err)
        .spawn()
        .map_err(|why| format!("could not be started ({why})"))?;
    // The child is waited for here, by the one that holds it: stopped through the same handle when
    // it is late, so the process is reaped and its id is never signalled once it is free.
    let deadline = Instant::now() + most;
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => crate::backoff(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("did not end within {}s", most.as_secs()));
            }
            Err(_) => break false,
        }
    };
    // The output is read through the handle it was written by: not by a path again.
    let mut out = vec![];
    said.seek(SeekFrom::Start(0))
        .and_then(|_| std::io::BufReader::new(said).read_to_end(&mut out))
        .map_err(|why| format!("left no output to read ({why})"))?;
    Ok(Ran {
        ok,
        out: String::from_utf8_lossy(&out).into_owned(),
    })
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
