//! `llx-desk mcp` end to end: a host speaks MCP to the binary on pipes, and the binary
//! relays to a daemon of a test build (no window). The connection survives a daemon that is
//! killed and started again, and calls never hang.

use std::path::Path;

use llx_desk::harness::mcp::Mcp;
use llx_desk::harness::{Home, asked};
use serde_json::json;

fn home(name: &str) -> Home {
    Home::new(env!("CARGO_BIN_EXE_llx-desk"), name)
}

/// Times short enough for a test: a ping each 200 ms, a call waits up to 12 s for a connection.
const FAST: [(&str, &str); 2] = [
    ("LLX_DESK_MCP_PING_MS", "200"),
    ("LLX_DESK_MCP_WAIT_MS", "12000"),
];

const SLEEPER: &str = "sleep 60";

#[test]
fn the_server_says_what_it_is_lists_six_tools_and_ignores_what_it_does_not_know() {
    let home = home("mcp-hello");
    let mut mcp = Mcp::start(&home, Path::new("/tmp"), &FAST);
    let hello = mcp.initialize();
    assert_eq!(hello["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(hello["result"]["serverInfo"]["name"], "llx-desk");
    assert!(hello["result"]["capabilities"]["tools"].is_object());

    let list = mcp.request("tools/list", &json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["open", "act", "look", "screenshot", "tabs", "comments"]
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(tool["annotations"]["readOnlyHint"].is_boolean(), "{tool}");
        assert!(tool["description"].as_str().unwrap().len() < 500, "{tool}");
    }
    let read_only: Vec<&str> = tools
        .iter()
        .filter(|t| t["annotations"]["readOnlyHint"] == true)
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(read_only, ["look", "screenshot"]);

    assert_eq!(mcp.request("ping", &json!({}))["result"], json!({}));
    // An unknown request is refused; an unknown notification is ignored; a tool that is none
    // is a protocol error.
    assert_eq!(
        mcp.request("resources/list", &json!({}))["error"]["code"],
        -32601
    );
    mcp.send(&json!({"jsonrpc": "2.0", "method": "notifications/something"}));
    let unknown = mcp.request("tools/call", &json!({"name": "nothing", "arguments": {}}));
    assert_eq!(unknown["error"]["code"], -32602);
    // The server did not touch a daemon for any of it.
    assert!(!home.socket().exists());
    assert_eq!(mcp.hang_up(), Some(0));
    assert_eq!(mcp.stderr(), "");
}

#[test]
fn asking_for_comments_with_no_daemon_answers_at_once_and_starts_none() {
    let home = home("mcp-nocomments");
    let mut mcp = Mcp::start(&home, Path::new("/tmp"), &FAST);
    mcp.initialize();
    let began = std::time::Instant::now();
    assert_eq!(
        mcp.text("comments", &json!({})),
        "No new comments from the person."
    );
    assert!(began.elapsed() < std::time::Duration::from_secs(5));
    assert!(!home.socket().exists(), "the daemon was started");
    assert_eq!(home.pid(), None);
    assert_eq!(mcp.hang_up(), Some(0));
}

#[test]
fn the_tools_work_in_a_terminal_and_the_peoples_comments_come_through() {
    let home = home("mcp-tools");
    let dir = Path::new("/tmp");
    let mut mcp = Mcp::start(&home, dir, &FAST);
    mcp.initialize();

    let opened = mcp.text("open", &json!({"kind": "terminal", "command": "sh -c 'printf \"name? \"; read n; echo hello $n; sleep 60'", "wait_for": "name?"}));
    assert!(
        opened.contains("t1") && opened.contains("name?"),
        "{opened}"
    );
    let typed = mcp.text(
        "act",
        &json!({"tab": "t1", "input": "bob\n", "wait_for": "hello bob"}),
    );
    assert!(typed.contains("hello bob"), "{typed}");
    assert!(
        mcp.text("look", &json!({"tab": "t1"}))
            .contains("hello bob")
    );
    assert!(mcp.text("tabs", &json!({})).contains("t1 (terminal)"));
    // A terminal has no picture: it answers with its screen, as text.
    let shot = mcp.tool("screenshot", &json!({"tab": "t1"}));
    assert!(
        shot["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("hello bob"),
        "{shot}"
    );

    // A call that fails is a result the model reads, not a protocol error.
    let missing = mcp.tool("act", &json!({"tab": "t9", "input": "x"}));
    assert_eq!(missing["isError"], true);
    assert!(
        missing["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("t9")
    );

    assert_eq!(
        mcp.text("comments", &json!({})),
        "No new comments from the person."
    );
    let (client, _) = home.client();
    asked(
        &client,
        "test/say",
        json!({"tab": "t1", "words": "the prompt is cut off"}),
    );
    let comments = mcp.text("comments", &json!({}));
    assert!(comments.contains("the prompt is cut off"), "{comments}");
    assert_eq!(
        mcp.text("comments", &json!({})),
        "No new comments from the person."
    );

    // The CLI shares the owner: its `--end` closes what the server opened.
    let ended = home.run(dir, &["window", "--end"]);
    assert!(String::from_utf8_lossy(&ended.stdout).contains("Closed 1 tab"));
}

#[test]
fn the_connection_comes_back_when_the_daemon_is_killed_and_started_again() {
    let home = home("mcp-reconnect");
    let dir = Path::new("/tmp");
    let mut mcp = Mcp::start(&home, dir, &FAST);
    mcp.initialize();
    assert!(mcp.text("tabs", &json!({})).contains("No tabs"));
    let first = home.pid().unwrap();
    let subscribed =
        || String::from_utf8_lossy(&home.run(dir, &["status"]).stdout).contains("subscribed: 1");
    Home::until("the relay subscribes", subscribed);

    // The daemon dies without a word. The next call waits for a new one and is answered.
    home.kill_daemon();
    let tabs = mcp.text("tabs", &json!({}));
    assert!(tabs.contains("No tabs"), "{tabs}");
    let second = home.pid().unwrap();
    assert_ne!(first, second);
    // The subscription is made again, without anyone asking.
    Home::until("the relay subscribes again", subscribed);

    // A call in flight when the daemon dies is made again, and ends: it never hangs.
    mcp.text("open", &json!({"kind": "terminal", "command": SLEEPER}));
    let id = {
        let id = 100;
        mcp.send(&json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": "act", "arguments": {"tab": "t1", "wait_for": "never shown", "yield_ms": 20000}}}));
        id
    };
    Home::until("the call is running", || home.pid().is_some());
    home.kill_daemon();
    let reply = mcp.reply(id);
    let text = reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    // The new daemon has no such tab: the answer says so.
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    assert!(text.contains("t1"), "{text}");
    assert_ne!(home.pid().unwrap(), second);
}

#[test]
fn a_call_fails_clearly_when_no_daemon_can_be_had() {
    let home = home("mcp-nodaemon");
    // A directory a daemon cannot use: the lock is a directory, so serve cannot take it.
    std::fs::create_dir_all(home.dir().join("desk.lock")).unwrap();
    let mut mcp = Mcp::start(
        &home,
        Path::new("/tmp"),
        &[("LLX_DESK_MCP_WAIT_MS", "1500")],
    );
    mcp.initialize();
    let began = std::time::Instant::now();
    let result = mcp.tool("tabs", &json!({}));
    assert_eq!(result["isError"], true, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("could not be reached") && text.contains("llx-desk window"),
        "{text}"
    );
    assert!(began.elapsed() < std::time::Duration::from_secs(10));
    // The stdio side is still up.
    assert_eq!(mcp.request("ping", &json!({}))["result"], json!({}));
}

#[test]
fn the_server_and_the_command_line_name_the_same_owner_by_the_hosts_project() {
    let home = home("mcp-owner");
    let project =
        std::path::PathBuf::from(format!("/tmp/ldk-{}-owner-project", std::process::id()));
    std::fs::create_dir_all(&project).unwrap();
    // The server starts where it likes; the host says the project.
    let mut mcp = Mcp::start(
        &home,
        Path::new("/"),
        &[("CLAUDE_PROJECT_DIR", project.to_str().unwrap())],
    );
    mcp.initialize();
    assert!(
        mcp.text("open", &json!({"kind": "terminal", "command": SLEEPER}))
            .contains("t1")
    );
    // A hook of the same host, running anywhere, closes it with the same variable.
    let mut end = home.command(Path::new("/"), &["window", "--end"]);
    let ended = Home::finish(end.env("CLAUDE_PROJECT_DIR", &project));
    assert!(
        String::from_utf8_lossy(&ended.stdout).contains("Closed 1 tab"),
        "{ended:?}"
    );
    // Without it, it is another owner's, and sees nothing.
    let other = home.run(Path::new("/"), &["window", "--end"]);
    assert!(String::from_utf8_lossy(&other.stdout).contains("Closed 0 tabs"));
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn a_new_build_takes_over_the_running_server_and_the_host_notices_nothing() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;
    let home = home("mcp-upgrade");
    // `from` copied to `to` as a new file put in place at once, as an install does.
    let install = |from: &Path, to: &Path| {
        let next = to.with_extension("next");
        std::fs::copy(from, &next).unwrap();
        std::fs::rename(&next, to).unwrap();
    };
    let bin = home.dir().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("llx-desk");
    let built = Path::new(home.exe());
    install(built, &exe);
    let watch = [("LLX_DESK_MCP_WATCH_MS", "100")];
    let mut mcp = Mcp::start_of(
        &home,
        &exe,
        Path::new("/tmp"),
        &[FAST[0], FAST[1], watch[0]],
    );
    mcp.initialize();
    assert!(
        mcp.hear(
            "notifications/tools/list_changed",
            Duration::from_millis(600)
        )
        .is_none(),
        "the same build took over"
    );

    // A build that does not answer is never run: the server goes on as it was.
    let broken = home.dir().join("broken");
    std::fs::write(&broken, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&broken, std::fs::Permissions::from_mode(0o755)).unwrap();
    install(&broken, &exe);
    assert!(
        mcp.hear("notifications/tools/list_changed", Duration::from_secs(1))
            .is_none()
    );
    assert_eq!(mcp.request("ping", &json!({}))["result"], json!({}));

    // A working build takes over on the same pipes, says the tools may have changed, and
    // answers as the old one did.
    install(built, &exe);
    assert!(
        mcp.hear("notifications/tools/list_changed", Duration::from_secs(10))
            .is_some(),
        "the new build did not take over"
    );
    assert_eq!(mcp.request("ping", &json!({}))["result"], json!({}));
    let list = mcp.request("tools/list", &json!({}));
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 6);
    assert_eq!(mcp.hang_up(), Some(0));
    // Only the build that did not answer is said; the take-over is quiet.
    let said = mcp.stderr();
    assert_eq!(said.lines().count(), 1, "{said}");
    assert!(said.contains("did not answer"), "{said}");
}
