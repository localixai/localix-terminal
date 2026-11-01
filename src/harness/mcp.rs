//! Test support: `llx-desk mcp` run as a host runs it, on pipes, spoken to in MCP.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test support: a panic fails the test"
)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Duration;

use serde_json::{Value, json};

use super::Home;

/// How long a reply may take in a test.
const REPLY: Duration = Duration::from_secs(20);

/// An MCP server process and the host's two ends of it.
pub struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next: u64,
    /// Notifications that came while a reply was awaited.
    heard: Vec<Value>,
}

impl Mcp {
    /// The server for `home`'s daemon, started in `cwd`, with its relay's times shortened by
    /// `envs`.
    pub fn start(home: &Home, cwd: &Path, envs: &[(&str, &str)]) -> Self {
        Self::start_of(home, Path::new(home.exe()), cwd, envs)
    }

    /// `start` for the binary at `exe`.
    pub fn start_of(home: &Home, exe: &Path, cwd: &Path, envs: &[(&str, &str)]) -> Self {
        let mut command = home.command_of(exe, cwd, &["mcp"]);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in envs {
            command.env(name, value);
        }
        let mut child = command.spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = sync_channel(256);
        std::thread::Builder::new()
            .name("mcp-host-read".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            })
            .unwrap();
        let stdin = child.stdin.take();
        Self {
            child,
            stdin,
            lines,
            next: 1,
            heard: vec![],
        }
    }

    /// The notifications that came while replies were awaited.
    pub fn heard(&self) -> &[Value] {
        &self.heard
    }

    /// A message to the server.
    pub fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    /// A request, and its reply (the whole message: `result` or `error`); notifications that
    /// come first are kept in `heard`.
    pub fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        self.reply(id)
    }

    /// The reply to request `id`.
    pub fn reply(&mut self, id: u64) -> Value {
        loop {
            let line = self
                .lines
                .recv_timeout(REPLY)
                .unwrap_or_else(|_| panic!("no reply to {id}"));
            let message: Value = serde_json::from_str(&line).unwrap_or_else(|why| {
                panic!("stdout carried a line that is no JSON ({why}): {line}")
            });
            if message["id"] == json!(id) {
                return message;
            }
            self.heard.push(message);
        }
    }

    /// The next notification of `method`, waited for; those before it are kept in `heard`.
    pub fn hear(&mut self, method: &str, within: Duration) -> Option<Value> {
        let until = std::time::Instant::now() + within;
        loop {
            let left = until.saturating_duration_since(std::time::Instant::now());
            let line = self.lines.recv_timeout(left).ok()?;
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["method"] == method {
                return Some(message);
            }
            self.heard.push(message);
        }
    }

    /// The initialize handshake, as a host does it.
    pub fn initialize(&mut self) -> Value {
        let hello = json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "test-host", "version": "0"},
        });
        let reply = self.request("initialize", &hello);
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        reply
    }

    /// A tool called: the reply's result (or error) as it came.
    pub fn tool(&mut self, name: &str, arguments: &Value) -> Value {
        let reply = self.request("tools/call", &json!({"name": name, "arguments": arguments}));
        reply.get("result").cloned().unwrap_or(reply)
    }

    /// A tool called, and the first text of its result.
    pub fn text(&mut self, name: &str, arguments: &Value) -> String {
        let result = self.tool(name, arguments);
        result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    /// The host closes its end: the server ends. Its exit code.
    pub fn hang_up(&mut self) -> Option<i32> {
        self.stdin = None;
        for _ in 0..100 {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    /// What the server wrote on its standard error so far, after it ended.
    pub fn stderr(&mut self) -> String {
        let mut text = String::new();
        if let Some(mut err) = self.child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut err, &mut text);
        }
        text
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
