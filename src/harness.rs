//! Test support, compiled only into test builds (the `test-server` feature, which nothing but
//! this crate's own dev-dependency turns on): a server directory of a test's own, and the binary
//! run against it. The lock and the socket live in `LLXTERM_TEST_DIR`, and a server of a test
//! build never opens a window, so no test touches the person's server or shows a thing.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test support: a panic fails the test; it waits for a server to exit, which has no event to wait on"
)]

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Duration;

use serde_json::Value;

use crate::client::{Client, Notes};

/// A directory for one test's server, which is stopped (by the pid in its lock) and removed when
/// the test ends.
pub struct Home {
    exe: &'static str,
    dir: PathBuf,
}

impl Home {
    /// `exe` is the binary under test (`env!("CARGO_BIN_EXE_llxterm")`).
    pub fn new(exe: &'static str, name: &str) -> Self {
        // Short: a unix socket's path is limited to about a hundred bytes.
        let dir = PathBuf::from(format!("/tmp/ldk-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self { exe, dir }
    }

    /// The binary under test.
    pub fn exe(&self) -> &'static str {
        self.exe
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn socket(&self) -> PathBuf {
        self.dir.join("desk.sock")
    }

    /// The binary with this home's environment: its directory for the lock and the socket, and
    /// the same for the files, so nothing reaches the person's.
    pub fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        self.command_of(Path::new(self.exe), cwd, args)
    }

    /// `command` for the binary at `exe` (a copy a test replaces).
    pub fn command_of(&self, exe: &Path, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(exe);
        command
            .args(args)
            .current_dir(cwd)
            .env("LLXTERM_TEST_DIR", &self.dir)
            .env("LLXTERM_HOME", &self.dir)
            .env("XDG_CONFIG_HOME", &self.dir)
            .env_remove("LLXTERM_PROJECT");
        command
    }

    pub fn run(&self, cwd: &Path, args: &[&str]) -> Output {
        Self::finish(&mut self.command(cwd, args))
    }

    /// `command` run to its end. The test's own timeout is the harness's: a call that hangs is
    /// a failed test.
    pub fn finish(command: &mut Command) -> Output {
        command.output().unwrap()
    }

    /// One `window` call: whether it came to what it asked, and what it printed.
    pub fn window(&self, cwd: &Path, json: &str) -> (bool, String) {
        let out = self.run(cwd, &["window", json]);
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        (out.status.success(), text)
    }

    /// A server started now, with `envs` added to its environment (a build of another name); it
    /// is not this test's child (it ends with `quit`, or when the test does). Returns once
    /// it listens.
    pub fn start(&self, envs: &[(&str, &str)]) {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("\"$0\" serve </dev/null >/dev/null 2>&1 &")
            .arg(self.exe)
            .env("LLXTERM_TEST_DIR", &self.dir)
            .env("LLXTERM_HOME", &self.dir)
            .env("XDG_CONFIG_HOME", &self.dir);
        for (name, value) in envs {
            command.env(name, value);
        }
        command.status().unwrap();
        Self::until("the server listens", || {
            UnixStream::connect(self.socket()).is_ok()
        });
    }

    /// Until `want` holds, within ten seconds.
    pub fn until(what: &str, want: impl Fn() -> bool) {
        for _ in 0..200 {
            if want() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("never: {what}");
    }

    /// A client of this home's server, handshake done, and the notifications it hears.
    pub fn client(&self) -> (Client, Receiver<(String, Value)>) {
        let (sender, received) = sync_channel(64);
        let notes: Notes = Arc::new(move |method, params| {
            let _ = sender.try_send((method.to_string(), params.clone()));
        });
        let stream = UnixStream::connect(self.socket()).unwrap();
        (Client::over(stream, &notes).unwrap(), received)
    }

    /// The pid the server's lock names, if there is one.
    pub fn pid(&self) -> Option<i32> {
        std::fs::read_to_string(self.dir.join("desk.lock"))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// Whether process `pid` exists.
    pub fn alive(pid: i32) -> bool {
        // SAFETY: signal 0 only asks whether the process exists.
        unsafe { libc::kill(pid, 0) == 0 }
    }

    /// The server killed outright (it gets no chance to clean up), waited for.
    pub fn kill_app(&self) {
        if let Some(pid) = self.pid() {
            // SAFETY: a signal to the pid this test's own lock names.
            unsafe { libc::kill(pid, libc::SIGKILL) };
            Self::wait_gone(pid);
        }
    }

    /// Until `pid` is gone, for five seconds.
    pub fn wait_gone(pid: i32) {
        for _ in 0..50 {
            if !Self::alive(pid) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let pid = self.pid();
        self.run(Path::new("/tmp"), &["quit"]);
        if let Some(pid) = pid {
            Self::wait_gone(pid);
            // SAFETY: as above; a server that did not stop is ended.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `method` asked of `client`, and what it answered; a failure fails the test.
pub fn asked(client: &Client, method: &str, params: Value) -> Value {
    client.request(method, params).unwrap().value
}
