//! Programs of the person's, run as their terminal would: found on their login shell's PATH (an
//! app started from the Finder has only the system's), each within a deadline and stopped past
//! it.

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What a program said and how it ended.
pub(super) struct Ran {
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
    "CODEX_HOME",
    "GEMINI_CLI_HOME",
    "COPILOT_HOME",
    "XDG_CONFIG_HOME",
    "HERMES_HOME",
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
/// not answer within a few seconds.
pub(super) fn login_path() -> Option<String> {
    // A login shell may take seconds to start (a version manager's setup), and every
    // integration's state asks for it, several at once: one asks, the others wait for its answer
    // (the PATH read before, when there is one, once they have waited a moment). The lock is held
    // for the bookkeeping only, never while the shell runs.
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

/// One of the agents' variables as a new terminal of the person's has it: their login shell's,
/// else this process's own (the app started from a terminal). None when it is not set.
pub(super) fn login_var(name: &str) -> Option<String> {
    let _ = login_path();
    let said = (crate::lock(&KNOWN).vars.iter())
        .find(|(known, _)| known == name)
        .map(|(_, value)| value.clone());
    said.or_else(|| std::env::var(name).ok())
        .filter(|value| !value.is_empty())
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

/// Whether `dir` is on `path`.
pub(super) fn on_path(path: &str, dir: &Path) -> bool {
    std::env::split_paths(path).any(|on| on == dir)
}

/// The program `name` on `path`.
pub(super) fn which(name: &str, path: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|program| program.is_file())
}

/// The program `name` on the login shell's PATH, and that PATH to run it with.
pub(super) fn find(name: &str) -> Result<(PathBuf, String), String> {
    let path = login_path().ok_or("the login shell did not say its PATH")?;
    let program =
        which(name, &path).ok_or_else(|| format!("not installed (no `{name}` on the PATH)"))?;
    Ok((program, path))
}

/// Why a program that failed did: what it said, or when it said nothing, `what` it means.
pub(super) fn failure(ran: &Ran, what: &str) -> String {
    match ran.out.trim() {
        "" => what.to_string(),
        said => said.to_string(),
    }
}

/// `program` with `args`, its PATH `path`, within `most`: what it said (stdout, then stderr).
pub(super) fn run(
    program: &Path,
    args: &[&str],
    path: &str,
    most: Duration,
) -> Result<Ran, String> {
    run_with(program, args, path, most, None)
}

/// As [`run`], with `input` for what the program asks (its standard input; none: nothing).
pub(super) fn run_with(
    program: &Path,
    args: &[&str],
    path: &str,
    most: Duration,
    input: Option<&str>,
) -> Result<Ran, String> {
    let mut command = Command::new(program);
    let vars = crate::lock(&KNOWN).vars.clone();
    command.args(args).env("PATH", path).envs(vars);
    within_given(command, most, input)
        .map_err(|why| format!("{} {} {why}", program.display(), args.join(" ")))
}

/// What `command` says, when it ends within `most`; one that does not is stopped. Its output
/// goes to a file, not a pipe: an interactive shell leaves helpers behind (a prompt's git
/// watcher) that would hold a pipe open long after the shell ended.
pub(super) fn within(command: Command, most: Duration) -> Result<Ran, String> {
    within_given(command, most, None)
}

/// A file of this run's own, made new (it never follows a link or takes over one that was there)
/// and for its owner only, under a name no other user can guess: the shared temporary directory
/// is another user's to write in too.
fn private_file(tag: &str) -> Result<(PathBuf, std::fs::File), String> {
    let path = std::env::temp_dir().join(format!("llx-desk-setup-{}-{tag}", crate::token()));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|why| format!("could not make its temporary file ({why})"))?;
    Ok((path, file))
}

/// As [`within`], `input` given as the program's standard input (a file too, never a pipe).
fn within_given(mut command: Command, most: Duration, input: Option<&str>) -> Result<Ran, String> {
    let mut made = vec![];
    let ran = within_files(&mut command, most, input, &mut made);
    for file in made {
        let _ = std::fs::remove_file(file);
    }
    ran
}

fn within_files(
    command: &mut Command,
    most: Duration,
    input: Option<&str>,
    made: &mut Vec<PathBuf>,
) -> Result<Ran, String> {
    let stdin = match input {
        Some(input) => {
            let (path, mut file) = private_file("in")?;
            made.push(path);
            let given = file
                .write_all(input.as_bytes())
                .and_then(|()| file.seek(SeekFrom::Start(0)));
            given.map_err(|why| format!("could not make its temporary file ({why})"))?;
            Stdio::from(file)
        }
        None => Stdio::null(),
    };
    let (path, out) = private_file("out")?;
    made.push(path);
    let err = out
        .try_clone()
        .map_err(|why| format!("could not make its temporary file ({why})"))?;
    let mut said = out
        .try_clone()
        .map_err(|why| format!("could not read its output ({why})"))?;
    crate::fds::as_started(command);
    let mut child = (command.stdin(stdin))
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
