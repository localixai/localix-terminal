//! The binary end to end against a daemon of its own (terminals only, no window): the CLI starts
//! it, keeps two owners' tabs apart, and stops it. Nothing here opens a window.

use std::path::Path;

use llx_desk::harness::Home;

fn home(name: &str) -> Home {
    Home::new(env!("CARGO_BIN_EXE_llx-desk"), name)
}

#[test]
fn help_and_the_tool_description_need_no_daemon() {
    let home = home("help");
    let tmp = Path::new("/tmp");
    let help = home.run(tmp, &[]);
    assert!(String::from_utf8_lossy(&help.stdout).contains("llx-desk window"));
    let describe = home.run(tmp, &["window"]);
    let text = String::from_utf8_lossy(&describe.stdout);
    assert!(
        text.contains("tabs of one window") && text.contains("\"properties\""),
        "{text}"
    );
    assert!(!home.socket().exists(), "a daemon was started");
}

#[test]
fn a_key_saved_once_is_kept_for_the_owner_alone_and_never_printed() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    let home = home("key");
    let mut child = home
        .command(Path::new("/tmp"), &["key"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"sk-or-v1-secret\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{text}");
    assert!(!text.contains("secret"), "{text}");
    let saved = home.dir().join("llx-desk").join("openrouter.key");
    assert!(text.contains(&*saved.to_string_lossy()), "{text}");
    assert_eq!(
        std::fs::read_to_string(&saved).unwrap(),
        "sk-or-v1-secret\n"
    );
    let mode = std::fs::metadata(&saved).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let empty = home.run(Path::new("/tmp"), &["key"]);
    assert_eq!(empty.status.code(), Some(3));
    assert!(!home.socket().exists(), "a daemon was started");
}

#[test]
fn a_bad_argument_is_exit_3_and_ending_without_a_daemon_is_quiet() {
    let home = home("bad");
    let tmp = Path::new("/tmp");
    assert_eq!(home.run(tmp, &["window", "[1]"]).status.code(), Some(3));
    assert_eq!(home.run(tmp, &["window", "--bogus"]).status.code(), Some(3));
    let end = home.run(tmp, &["window", "--end"]);
    assert_eq!(end.status.code(), Some(0));
    assert!(end.stdout.is_empty() && !home.socket().exists());
}

#[test]
fn two_owners_work_in_terminals_of_their_own() {
    let home = home("two");
    let (a, b) = (Path::new("/tmp"), Path::new("/var"));
    let program = r#"sh -c 'printf "name? "; read n; echo hello $n; sleep 30'"#;
    let open = serde_json::json!({"action": "open", "kind": "terminal", "command": program, "wait_for": "name?"});
    let (ok, text) = home.window(a, &open.to_string());
    assert!(
        ok && text.contains("t1") && text.contains("name?"),
        "{text}"
    );

    let type_in = r#"{"action":"interact","tab":"t1","input":"bob\n","wait_for":"hello bob"}"#;
    let (ok, text) = home.window(a, type_in);
    assert!(ok && text.contains("hello bob"), "{text}");

    // The other owner sees none of it, and cannot act in it.
    let (ok, text) = home.window(b, r#"{"action":"list"}"#);
    assert!(ok && text.contains("No tabs"), "{text}");
    let (ok, text) = home.window(b, type_in);
    assert!(!ok && text.contains("among yours"), "{text}");
    let (ok, text) = home.window(a, r#"{"action":"list"}"#);
    assert!(ok && text.contains("t1 (terminal)"), "{text}");

    let status = home.run(a, &["status"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("terminals: 1"));

    let end = home.run(a, &["window", "--end"]);
    assert!(String::from_utf8_lossy(&end.stdout).contains("Closed 1 tab"));
    let (_, text) = home.window(a, r#"{"action":"list"}"#);
    assert!(text.contains("No tabs"), "{text}");
}

#[test]
fn a_program_that_ends_is_not_kept_and_a_goal_without_a_key_says_so() {
    let home = home("ends");
    let tmp = Path::new("/tmp");
    let (ok, text) = home.window(
        tmp,
        r#"{"action":"open","kind":"terminal","command":"echo done-here"}"#,
    );
    assert!(
        ok && text.contains("done-here") && text.contains("ended"),
        "{text}"
    );
    let (ok, text) = home.window(tmp, r#"{"action":"list"}"#);
    assert!(ok && text.contains("No tabs"), "{text}");
    let (ok, text) = home.window(
        tmp,
        r#"{"action":"open","kind":"terminal","command":"sleep 20","goal":"x"}"#,
    );
    assert!(ok && text.contains("OPENROUTER_API_KEY"), "{text}");
}

#[test]
fn the_daemon_stops_and_its_socket_goes() {
    let home = home("stop");
    let tmp = Path::new("/tmp");
    let (ok, _) = home.window(tmp, r#"{"action":"list"}"#);
    assert!(ok);
    let pid = home.pid().unwrap();
    assert!(home.socket().exists());
    let stopped = home.run(tmp, &["serve", "stop"]);
    assert!(stopped.status.success());
    Home::wait_gone(pid);
    assert!(!Home::alive(pid), "the daemon still runs");
    assert!(!home.socket().exists());
}

#[test]
fn a_second_serve_is_quiet_and_there_stays_one_daemon() {
    let home = home("second");
    let tmp = Path::new("/tmp");
    assert!(home.window(tmp, r#"{"action":"list"}"#).0);
    let pid = home.pid().unwrap();
    for _ in 0..3 {
        let again = home.run(tmp, &["serve"]);
        assert_eq!(again.status.code(), Some(0));
        assert!(again.stdout.is_empty(), "{:?}", again.stdout);
        assert!(again.stderr.is_empty(), "{:?}", again.stderr);
    }
    assert_eq!(home.pid(), Some(pid));
    assert!(Home::alive(pid));
    assert!(home.window(tmp, r#"{"action":"list"}"#).0);
}

#[test]
fn clients_racing_to_start_a_daemon_end_with_one() {
    let home = home("race");
    let tmp = Path::new("/tmp");
    let calls: Vec<_> = (0..8)
        .map(|_| {
            home.command(tmp, &["window", r#"{"action":"list"}"#])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for call in calls {
        let out = call.wait_with_output().unwrap();
        assert!(out.status.success(), "{:?}", out);
    }
    // The lock names the one that serves; every client reached it.
    let pid = home.pid().unwrap();
    let status = home.run(tmp, &["status"]);
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains(&format!("pid {pid}")), "{text}");
}

#[test]
fn the_home_places_files_only_and_the_socket_stays_where_every_client_finds_it() {
    let home = home("files");
    let other =
        std::path::PathBuf::from(format!("/tmp/ldk-{}-files-elsewhere", std::process::id()));
    std::fs::create_dir_all(&other).unwrap();
    let mut call = home.command(Path::new("/tmp"), &["window", r#"{"action":"list"}"#]);
    let out = Home::finish(call.env("LLX_DESK_HOME", &other));
    assert!(out.status.success(), "{out:?}");
    assert!(home.socket().exists(), "the socket moved with the home");
    assert!(!other.join("desk.sock").exists());
    assert!(!other.join("desk.lock").exists());
    // The log, the comments and the pictures go to the home.
    assert!(other.join("daemon.log").exists());
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn a_test_build_without_its_directory_reaches_nothing() {
    let home = home("unset");
    let mut call = home.command(Path::new("/tmp"), &["window", r#"{"action":"list"}"#]);
    let out = Home::finish(
        call.env_remove("LLX_DESK_TEST_DIR")
            .env_remove("LLX_DESK_HOME"),
    );
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    assert!(!Path::new("/nonexistent").exists());
}

#[test]
fn a_daemon_of_another_build_is_reached_and_a_second_serve_starts_nothing() {
    let home = home("builds");
    let tmp = Path::new("/tmp");
    home.start(&[("LLX_DESK_BUILD", "build-a")]);
    let pid = home.pid().unwrap();
    let mut again = home.command(tmp, &["serve"]);
    let again = Home::finish(again.env("LLX_DESK_BUILD", "build-b"));
    assert_eq!(again.status.code(), Some(0));
    assert!(
        again.stdout.is_empty() && again.stderr.is_empty(),
        "{again:?}"
    );
    // The clients reach the one that runs; it is not replaced.
    let (ok, text) = home.window(tmp, r#"{"action":"list"}"#);
    assert!(ok, "{text}");
    let status = home.run(tmp, &["status"]);
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.contains(&format!("pid {pid}")) && status.contains("build: build-a"),
        "{status}"
    );
}

#[test]
fn an_older_daemon_is_named_never_joined_by_a_second_and_can_be_stopped() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    let home = home("older");
    let tmp = Path::new("/tmp");
    // What protocol 1 did with a line it did not know: said so, and closed.
    let listener = UnixListener::bind(home.socket()).unwrap();
    let old = std::thread::Builder::new()
        .name("old-daemon".into())
        .spawn(move || {
            let mut stopped = false;
            while !stopped {
                let (stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                stopped = line.contains("\"op\":\"stop\"");
                let said = if stopped {
                    "{\"ok\":true,\"text\":\"stopping\"}\n"
                } else {
                    "{\"ok\":false,\"text\":\"not a frame: expected value\"}\n"
                };
                (&stream).write_all(said.as_bytes()).unwrap();
            }
        })
        .unwrap();
    let (ok, text) = home.window(tmp, r#"{"action":"list"}"#);
    assert!(!ok && text.contains("older protocol"), "{text}");
    // No second daemon was started beside it.
    assert_eq!(home.pid(), None);
    let stopped = home.run(tmp, &["serve", "stop"]);
    assert_eq!(String::from_utf8_lossy(&stopped.stdout).trim(), "stopping");
    old.join().unwrap();
}

#[test]
fn a_listener_that_closes_without_a_word_is_not_called_an_older_protocol() {
    use std::os::unix::net::UnixListener;
    let home = home("silent");
    let tmp = Path::new("/tmp");
    let listener = UnixListener::bind(home.socket()).unwrap();
    let hang_up = std::thread::Builder::new()
        .name("silent-listener".into())
        .spawn(move || {
            // Accepts and closes, as a daemon dying in its handshake would.
            let _ = listener.accept();
        })
        .unwrap();
    let (ok, text) = home.window(tmp, r#"{"action":"list"}"#);
    assert!(!ok, "{text}");
    assert!(!text.contains("older protocol"), "{text}");
    hang_up.join().unwrap();
}

#[test]
fn status_says_when_no_daemon_runs_and_stop_is_answered_before_it_ends() {
    let home = home("status");
    let tmp = Path::new("/tmp");
    let none = home.run(tmp, &["serve", "status"]);
    let said =
        String::from_utf8_lossy(&none.stdout).to_string() + &String::from_utf8_lossy(&none.stderr);
    assert!(said.contains("no daemon is running"), "{said}");
    home.start(&[]);
    let pid = home.pid().unwrap();
    let stopped = home.run(tmp, &["serve", "stop"]);
    assert_eq!(String::from_utf8_lossy(&stopped.stdout).trim(), "stopping");
    Home::wait_gone(pid);
}

#[test]
fn follow_prints_a_comment_when_it_is_sent_and_a_daemon_that_is_gone_is_waited_for() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let home = home("follow");
    let tmp = Path::new("/tmp");
    let open = r#"{"action":"open","kind":"terminal","command":"sleep 60","yield_ms":300}"#;
    assert!(home.window(tmp, open).0);
    let mut follow = home
        .command(tmp, &["window", "--said", "--follow"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let (lines, printed) = std::sync::mpsc::sync_channel::<String>(16);
    let out = follow.stdout.take().unwrap();
    std::thread::Builder::new()
        .name("follow-reader".into())
        .spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = lines.send(line);
            }
        })
        .unwrap();
    // It subscribes (the status counts it) rather than asking again and again.
    let subscribed = || {
        let status = home.run(tmp, &["status"]);
        String::from_utf8_lossy(&status.stdout).contains("subscribed: 1")
    };
    Home::until("the follower subscribes", subscribed);
    let said = |words: &str| {
        let (client, _) = home.client();
        client
            .request("test/say", serde_json::json!({"tab": "t1", "words": words}))
            .unwrap();
    };
    said("the title is cut off");
    let mut block = String::new();
    while !block.contains("the title is cut off") {
        block += &printed
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }
    // The daemon goes away and comes back: the follower subscribes again, and goes on.
    home.kill_daemon();
    home.start(&[]);
    assert!(home.window(tmp, open).0);
    Home::until("the follower subscribes again", subscribed);
    said("and the footer");
    let mut block = String::new();
    while !block.contains("and the footer") {
        block += &printed
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }
    follow.kill().unwrap();
    follow.wait().unwrap();
}

#[test]
fn follow_json_gives_each_note_whole_on_one_line() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let home = home("follow-json");
    let tmp = Path::new("/tmp");
    let open = r#"{"action":"open","kind":"terminal","command":"sleep 60","yield_ms":300}"#;
    assert!(home.window(tmp, open).0);
    let mut follow = home
        .command(tmp, &["window", "--said", "--follow", "--json"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let (lines, printed) = std::sync::mpsc::sync_channel::<String>(16);
    let out = follow.stdout.take().unwrap();
    std::thread::Builder::new()
        .name("follow-json-reader".into())
        .spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = lines.send(line);
            }
        })
        .unwrap();
    let subscribed = || {
        let status = home.run(tmp, &["status"]);
        String::from_utf8_lossy(&status.stdout).contains("subscribed: 1")
    };
    Home::until("the follower subscribes", subscribed);
    let (client, _) = home.client();
    // A note of several lines, its picture's path last: as a comment with its picture comes.
    let words = "make it red\nPicture of it: /tmp/shot.jpg";
    client
        .request("test/say", serde_json::json!({"tab": "t1", "words": words}))
        .unwrap();
    let line = printed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let block: String = serde_json::from_str(&line).unwrap();
    assert!(
        block.starts_with("### What the person sent from the window"),
        "{block}"
    );
    assert!(
        block.contains("make it red\nPicture of it: /tmp/shot.jpg"),
        "{block}"
    );
    follow.kill().unwrap();
    follow.wait().unwrap();
}

#[test]
fn a_secret_saved_once_is_typed_into_a_terminal_and_no_answer_shows_it() {
    use std::io::Write as _;
    use std::process::Stdio;
    let home = home("secret");
    let tmp = Path::new("/tmp");
    let mut child = home
        .command(tmp, &["secret", "PW"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"tr0ub4dor&3\n")
        .unwrap();
    let saved = child.wait_with_output().unwrap();
    let said = String::from_utf8_lossy(&saved.stdout);
    assert!(
        saved.status.success() && !said.contains("tr0ub4dor"),
        "{said}"
    );
    let listed = home.run(tmp, &["secret"]);
    assert_eq!(
        String::from_utf8_lossy(&listed.stdout).trim(),
        "{{secret:PW}}"
    );

    let program = r#"sh -c 'printf "password? "; read p; echo "got $p"; sleep 30'"#;
    let open = serde_json::json!({"action": "open", "kind": "terminal", "command": program, "wait_for": "password?"});
    let (ok, text) = home.window(tmp, &open.to_string());
    assert!(ok, "{text}");
    let typed = r#"{"action":"interact","tab":"t1","input":"{{secret:PW}}\n","wait_for":"got"}"#;
    let (ok, text) = home.window(tmp, typed);
    assert!(ok && text.contains("got {{secret:PW}}"), "{text}");
    assert!(!text.contains("tr0ub4dor"), "{text}");

    let missing = r#"{"action":"interact","tab":"t1","input":"{{secret:NOPE}}\n"}"#;
    let (ok, text) = home.window(tmp, missing);
    assert!(
        !ok && text.contains("llx-desk secret NOPE < FILE"),
        "{text}"
    );
    let _ = home.run(tmp, &["window", "--end"]);
}
