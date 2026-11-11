//! The program in front in a terminal: the leader of its foreground process group (or the
//! command a shell that only runs one waits for), by the name it was started as (`claude`, `codex`), so a program the person typed in a shell is known as
//! well as one the terminal was opened with. A script run by its runtime (`node …/claude`) is
//! known by the script. The reader asks for the group with each output (one cheap call) and for
//! the name only when the group changed: a program started, or ended, in front.

use std::os::fd::RawFd;

/// Runtimes a program may be a script of: the script names it.
const RUNTIMES: [&str; 6] = ["node", "bun", "deno", "python", "python3", "ruby"];

/// The program in front in the terminal behind `master`, if any (the app asks for the group and
/// the name apart: `fronted` reads the name only when the group changed).
#[cfg(test)]
pub(super) fn of(master: RawFd) -> Option<String> {
    name(group(master)?)
}

/// The terminal's foreground process group behind `master`, if any.
pub(super) fn group(master: RawFd) -> Option<libc::pid_t> {
    // SAFETY: tcgetpgrp only reads the terminal's foreground group, on a descriptor kept open.
    let group = unsafe { libc::tcgetpgrp(master) };
    (group > 0).then_some(group)
}

/// The name of the program in front of group `group`: its leader's, or past a shell that only
/// runs a command (`sh -c …`, as a terminal opened with one starts) and waits for it, that
/// command's. macOS's `sh` becomes the command itself; Linux's `dash` stays and waits.
pub(super) fn name(group: libc::pid_t) -> Option<String> {
    let mut args = arguments(group)?;
    let mut pid = group;
    // A few levels at most: a shell that runs a shell that runs the program.
    for _ in 0..4 {
        if !runs_a_command(&args) {
            break;
        }
        let Some(child) = last_child(pid) else {
            break;
        };
        let Some(next) = arguments(child) else {
            break;
        };
        (pid, args) = (child, next);
    }
    named(&args)
}

/// Whether `args` are a shell's that runs one command string (`sh -c …`) rather than a person's.
pub(super) fn runs_a_command(args: &[String]) -> bool {
    let shell = args
        .first()
        .map(|arg| arg.rsplit('/').next().unwrap_or(arg));
    matches!(shell, Some("sh" | "dash" | "bash" | "zsh"))
        && args.get(1).is_some_and(|arg| arg == "-c")
}

/// Process `pid`'s newest child (Linux: from `/proc`); none where it cannot be told.
#[cfg(not(target_os = "macos"))]
fn last_child(pid: libc::pid_t) -> Option<libc::pid_t> {
    let children = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).ok()?;
    children
        .split_whitespace()
        .rev()
        .find_map(|child| child.parse().ok())
}

/// macOS's `sh -c` becomes its command: it has no shell left to look past.
#[cfg(target_os = "macos")]
const fn last_child(_pid: libc::pid_t) -> Option<libc::pid_t> {
    None
}

/// The name process `pid` was started as.
pub(super) fn program(pid: libc::pid_t) -> Option<String> {
    named(&arguments(pid)?)
}

/// The name a program was started as, from its arguments.
pub(super) fn named(args: &[String]) -> Option<String> {
    let base = |arg: &str| arg.rsplit('/').next().unwrap_or(arg).to_string();
    let first = base(args.first()?);
    let first = first
        .strip_prefix('-')
        .map_or_else(|| first.clone(), str::to_string);
    if !RUNTIMES.contains(&first.as_str()) {
        return Some(first);
    }
    let script = args.iter().skip(1).find(|arg| !arg.starts_with('-'));
    Some(script.map_or(first, |script| base(script)))
}

/// Process `pid`'s arguments (Linux: from `/proc`).
#[cfg(not(target_os = "macos"))]
fn arguments(pid: libc::pid_t) -> Option<Vec<String>> {
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let args: Vec<String> = (line.split(|byte| *byte == 0))
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect();
    (!args.is_empty()).then_some(args)
}

/// Process `pid`'s arguments (macOS: `KERN_PROCARGS2`, its count, its program's path, then
/// the arguments, each ended by a NUL).
#[cfg(target_os = "macos")]
fn arguments(pid: libc::pid_t) -> Option<Vec<String>> {
    let mut most: libc::c_int = 0;
    let mut size = size_of::<libc::c_int>();
    let mut ask = [libc::CTL_KERN, libc::KERN_ARGMAX];
    // SAFETY: sysctl writes one c_int into `most`, whose size `size` says.
    let asked = unsafe {
        libc::sysctl(
            ask.as_mut_ptr(),
            2,
            (&raw mut most).cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if asked != 0 || most <= 0 {
        return None;
    }
    let mut buffer = vec![0_u8; usize::try_from(most).ok()?];
    let mut size = buffer.len();
    let mut ask = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    // SAFETY: sysctl writes at most `size` bytes into `buffer`, and says how many in `size`.
    let asked = unsafe {
        libc::sysctl(
            ask.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if asked != 0 {
        return None;
    }
    let read = buffer.get(..size)?;
    let count = read.get(..4)?;
    let count = usize::try_from(i32::from_ne_bytes(count.try_into().ok()?)).ok()?;
    // The program's path, its padding, then the arguments: the empty pieces are the padding.
    let args: Vec<String> = (read.get(4..)?.split(|byte| *byte == 0))
        .filter(|piece| !piece.is_empty())
        .skip(1)
        .take(count)
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect();
    (!args.is_empty()).then_some(args)
}

#[cfg(test)]
#[path = "foreground_tests.rs"]
mod tests;

/// Where process `pid` is now (its working directory): the folder a shell has `cd`'d to, or the
/// one a program in front runs in.
#[cfg(target_os = "macos")]
pub(super) fn cwd(pid: libc::pid_t) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut info = std::mem::MaybeUninit::<libc::proc_vnodepathinfo>::zeroed();
    let size = libc::c_int::try_from(size_of::<libc::proc_vnodepathinfo>()).ok()?;
    // SAFETY: proc_pidinfo writes at most `size` bytes into `info`, and says how many.
    let wrote = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if wrote != size {
        return None;
    }
    // SAFETY: proc_pidinfo filled the whole of `info`, as `wrote` says.
    let info = unsafe { info.assume_init() };
    // The path is a C string, in rows of 32 (libc's spelling of a MAXPATHLEN array).
    let path = info.pvi_cdir.vip_path;
    let bytes: Vec<u8> = (path.as_flattened().iter())
        .take_while(|byte| **byte != 0)
        .map(|byte| byte.to_ne_bytes()[0])
        .collect();
    (!bytes.is_empty()).then(|| std::ffi::OsStr::from_bytes(&bytes).into())
}

/// Where process `pid` is now (its working directory), from `/proc`.
#[cfg(not(target_os = "macos"))]
pub(super) fn cwd(pid: libc::pid_t) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}
