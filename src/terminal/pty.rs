//! A program in a pseudo-terminal of its own: the shell leads a new session whose controlling
//! terminal is the PTY, so Ctrl-C reaches its foreground group as a terminal's would.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use super::{COLUMNS, ROWS, environment, hooks};

/// The program started and the master end of its terminal.
pub(super) struct Pty {
    pub(super) child: Child,
    pub(super) master: File,
}

/// Start `command` (the user's shell when empty, as a login shell, as a terminal app starts it:
/// it reads the person's profile, so an app started from the Finder still gives them their
/// PATH) in `dir`. The environment comes from `vars`, else this process, with the terminal's own
/// identity and color capabilities applied before the shell reads the person's profile.
pub(super) fn spawn(
    command: &str,
    dir: &Path,
    vars: Option<&[(String, String)]>,
) -> io::Result<Pty> {
    let (master, slave) = open()?;
    let shell = vars
        .and_then(|vars| vars.iter().find(|(name, _)| name == "SHELL"))
        .map(|(_, shell)| shell.clone())
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "sh".into());
    // The person's shell by its name, when it is what starts.
    let own = command.trim().is_empty().then(|| {
        Path::new(&shell)
            .file_name()
            .map_or_else(|| "sh".into(), |name| name.to_string_lossy().into_owned())
    });
    let mut program = match &own {
        Some(name) => {
            let mut login = Command::new(&shell);
            login.arg0(format!("-{name}"));
            login
        }
        None => {
            let mut sh = Command::new("sh");
            sh.arg("-c").arg(command);
            sh
        }
    };
    environment::apply(&mut program, vars);
    if let Some(name) = &own {
        hooks::apply(&mut program, name, vars);
    }
    program
        .current_dir(dir)
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave));
    // SAFETY: between fork and exec the closure calls only async-signal-safe functions and
    // allocates nothing.
    unsafe { program.pre_exec(take_terminal) };
    crate::fds::as_started(&mut program);
    let child = program.spawn();
    // Every copy of the slave end is dropped here: while one is open in this process, the
    // reader would never see the end of the output.
    drop(program);
    let mut child = child?;
    if let Err(why) = set_nonblocking(master.as_raw_fd()) {
        // Started, but unusable: not left running with no tab.
        kill(&mut child);
        return Err(why);
    }
    Ok(Pty {
        child,
        master: File::from(master),
    })
}

/// The new session's leader takes the terminal on stdin as its own.
fn take_terminal() -> io::Result<()> {
    // SAFETY: setsid takes nothing and only makes the forked child a session leader.
    if unsafe { libc::setsid() } == -1 {
        return Err(io::Error::last_os_error());
    }
    for signal in [libc::SIGINT, libc::SIGQUIT] {
        // SAFETY: signal is async-signal-safe; it resets one disposition of the forked child
        // (an ignored signal would stay ignored across exec, and Ctrl-C would do nothing).
        unsafe { libc::signal(signal, libc::SIG_DFL) };
    }
    // SAFETY: ioctl is async-signal-safe and only reads its arguments.
    match unsafe { libc::ioctl(0, libc::TIOCSCTTY as _, 0) } {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

/// A terminal of `COLUMNS`×`ROWS`: its master and slave ends.
fn open() -> io::Result<(OwnedFd, OwnedFd)> {
    let (mut master, mut slave): (RawFd, RawFd) = (-1, -1);
    let mut size = libc::winsize {
        ws_row: ROWS,
        ws_col: COLUMNS,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: openpty writes two descriptors into the integers it is given, and reads the size.
    let made = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    };
    if made == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openpty just returned these two descriptors, which nothing else owns.
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    // SAFETY: as above.
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    for fd in [&master, &slave] {
        // SAFETY: fcntl on a descriptor this function owns.
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    Ok((master, slave))
}

/// Give the terminal behind `master` a new size; the kernel signals its program (SIGWINCH).
pub(super) fn resize(master: &File, columns: u16, rows: u16) -> io::Result<()> {
    let size = libc::winsize {
        ws_row: rows,
        ws_col: columns,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: TIOCSWINSZ only reads the winsize it is given, on a descriptor `master` keeps open.
    match unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ as _, &raw const size) } {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl on a descriptor the caller owns.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: as above.
    match unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

/// Kill the program and everything in its process group; the child is reaped.
pub(super) fn kill(child: &mut Child) {
    let leader = child.id().cast_signed();
    // SAFETY: kill only sends a signal; it touches nothing in this process.
    unsafe { libc::kill(-leader, libc::SIGKILL) };
    let _ = child.kill();
    let _ = child.wait();
}

/// How the terminal behind `master` takes typing now: whether it shows it, and whether it gathers
/// whole lines (a full-screen program or a shell's own line editor takes each key itself).
#[derive(Clone, Copy)]
pub(super) struct Typing {
    pub(super) shown: bool,
    pub(super) by_line: bool,
}

impl Typing {
    pub(super) fn of(master: &File) -> Option<Self> {
        // SAFETY: termios is a plain C struct, for which all zeros is a valid value.
        let mut modes: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: tcgetattr only writes into `modes`, on a descriptor `master` keeps open.
        (unsafe { libc::tcgetattr(master.as_raw_fd(), &raw mut modes) } == 0).then_some(Self {
            shown: modes.c_lflag & libc::ECHO != 0,
            by_line: modes.c_lflag & libc::ICANON != 0,
        })
    }

    /// Whether a line is read that is not shown: how a password is asked for.
    pub(super) fn hidden(self) -> bool {
        !self.shown && self.by_line
    }
}

/// Whether the program at the terminal behind `master` reads a line it does not show.
pub(super) fn hides_input(master: &File) -> bool {
    Typing::of(master).is_some_and(Typing::hidden)
}

/// Wait up to `ms` for `fd` to be readable.
pub(super) fn readable(fd: RawFd, ms: i32) -> bool {
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll reads and writes the one pollfd it is given.
    unsafe { libc::poll(&mut poll, 1, ms) > 0 }
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
