//! A system tool run for an update (`curl`, `ditto`, `codesign`): within a deadline, stopped
//! when it is late, its failure told in the last line it said.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// `command` run within `most`, quiet; `while_running` is called as it runs (a download's
/// progress is read then). An error says how it failed: what it said last, or how it ended.
pub(super) fn within(
    command: Command,
    most: Duration,
    while_running: &dyn Fn(),
) -> Result<(), String> {
    within_into(command, Stdio::null(), most, while_running)
}

/// [`within`], what the command says written to `out` (a file: a pipe nobody reads would
/// hold a long answer back).
pub(super) fn within_into(
    mut command: Command,
    out: Stdio,
    most: Duration,
    while_running: &dyn Fn(),
) -> Result<(), String> {
    let name = command.get_program().to_string_lossy().into_owned();
    let name = name.rsplit('/').next().unwrap_or(&name).to_string();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("{name} did not run: {why}"))?;
    // The child is waited for by the one that holds it, and stopped through the same handle.
    let deadline = Instant::now() + most;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                while_running();
                crate::backoff(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name} did not end within {}s", most.as_secs()));
            }
            Err(why) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name} was lost: {why}"));
            }
        }
    };
    if status.success() {
        return Ok(());
    }
    // A tool that fails says why in a few lines: far less than a pipe holds.
    let mut said = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut said);
    }
    let last = said.lines().rfind(|line| !line.trim().is_empty());
    let why = last.map_or_else(|| status.to_string(), |line| line.trim().to_string());
    Err(format!("{name} failed: {why}"))
}
