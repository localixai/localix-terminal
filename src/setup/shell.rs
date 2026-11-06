//! Programs of the person's, run as their terminal would: found on their login shell's PATH (an
//! app started from the Finder has only the system's), each within a deadline and stopped past
//! it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// What a program said and how it ended.
pub(super) struct Ran {
    pub ok: bool,
    pub out: String,
}

/// The PATH a new terminal of the person's has: their login shell's. None when the shell does
/// not answer within a few seconds.
pub(super) fn login_path() -> Option<String> {
    // Asked once a process: a login shell may take seconds to start (a version manager's setup),
    // and every integration's state asks for it, several at once. One that did not answer is
    // asked again next time.
    static KNOWN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    let mut known = crate::lock(&KNOWN);
    if known.is_none() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut asked = Command::new(&shell);
        asked.args(["-l", "-i", "-c", "printf %s \"$PATH\""]);
        let ran = within(asked, Duration::from_secs(30))?;
        *known = ran.ok.then_some(ran.out).filter(|path| !path.is_empty());
    }
    known.clone()
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
    command.args(args).env("PATH", path);
    within_given(command, most, input).ok_or_else(|| {
        format!(
            "{} {} did not end within {}s",
            program.display(),
            args.join(" "),
            most.as_secs()
        )
    })
}

/// What `command` says, when it ends within `most`; one that does not is stopped. Its output
/// goes to a file, not a pipe: an interactive shell leaves helpers behind (a prompt's git
/// watcher) that would hold a pipe open long after the shell ended.
pub(super) fn within(command: Command, most: Duration) -> Option<Ran> {
    within_given(command, most, None)
}

/// As [`within`], `input` given as the program's standard input (a file too, never a pipe).
fn within_given(mut command: Command, most: Duration, input: Option<&str>) -> Option<Ran> {
    static ASKED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = ASKED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let file = std::env::temp_dir().join(format!("llx-desk-setup-{}-{n}", std::process::id()));
    let given = std::env::temp_dir().join(format!("llx-desk-setup-{}-{n}.in", std::process::id()));
    let stdin = match input {
        Some(input) => {
            std::fs::write(&given, input).ok()?;
            Stdio::from(std::fs::File::open(&given).ok()?)
        }
        None => Stdio::null(),
    };
    let out = std::fs::File::create(&file).ok()?;
    let err = out.try_clone().ok()?;
    let spawned = (command.stdin(stdin)).stdout(out).stderr(err).spawn();
    let ran = spawned.ok().and_then(|mut child| {
        let pid = child.id();
        let (done, ended) = std::sync::mpsc::sync_channel(1);
        let waiting = std::thread::Builder::new()
            .name("desk-setup-run".into())
            .spawn(move || {
                let status = child.wait();
                let _ = done.send(status.is_ok_and(|status| status.success()));
            });
        let ended = match waiting {
            Ok(_) => ended.recv_timeout(most).ok(),
            Err(_) => None,
        };
        if ended.is_none() {
            // Not reaped yet (its waiter still waits), so the id is still the child's.
            // SAFETY: kill only sends a signal.
            unsafe { libc::kill(pid.cast_signed(), libc::SIGKILL) };
        }
        let ok = ended?;
        let out = String::from_utf8_lossy(&std::fs::read(&file).ok()?).into_owned();
        Some(Ran { ok, out })
    });
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&given);
    ran
}

#[cfg(test)]
mod tests {
    use super::{on_path, within};
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn a_shell_that_leaves_a_helper_behind_still_answers_at_once() {
        let started = std::time::Instant::now();
        let mut shell = std::process::Command::new("/bin/sh");
        shell.args(["-c", "sleep 30 & printf hi"]);
        let ran = within(shell, Duration::from_secs(5)).unwrap();
        assert!(ran.ok);
        assert_eq!(ran.out, "hi");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        // One that does not end in time is stopped, and says nothing.
        let mut slow = std::process::Command::new("/bin/sh");
        slow.args(["-c", "sleep 30"]);
        assert!(within(slow, Duration::from_millis(200)).is_none());
        // One that fails says so.
        let mut failing = std::process::Command::new("/bin/sh");
        failing.args(["-c", "echo no; exit 3"]);
        assert!(!within(failing, Duration::from_secs(5)).unwrap().ok);
    }

    #[test]
    fn a_directory_is_on_a_path_by_its_whole_name() {
        assert!(on_path(
            "/usr/bin:/Users/me/.local/bin",
            Path::new("/Users/me/.local/bin")
        ));
        assert!(!on_path("/usr/bin", Path::new("/Users/me/.local/bin")));
    }
}
