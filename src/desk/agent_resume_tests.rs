//! Tests of `agent_resume`: a terminal's coding agent kept as it is seen in front and as its
//! hooks name its session, and let go of when it ends, but not as the app quits.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::desk::Desk;
use crate::desk::tests::{caller, desk_on};
use crate::ledger;
use crate::testing::Scratch;

pub(in crate::desk) const ID: &str = "0b8f6a52-6c1e-4b5e-9a57-3d5f0c2e9a11";

/// A stand-in for Claude Code in `bin`: it says what it was started with, then waits for a line
/// under its own name, as the agent waits for the person (`exit` set: it ends with that code at
/// once, as one whose session is not found).
pub(in crate::desk) fn stand_in(bin: &Path, exit: Option<u8>) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(bin).unwrap();
    // It speaks again after it is the agent by name, as an agent draws: the terminal reads who
    // is in front as output comes, and the name again only a while after.
    let said = r#"echo "stand-in claude $*""#;
    let script = match exit {
        Some(code) => format!("#!/bin/bash\n{said}\nexit {code}\n"),
        None => format!(
            "#!/bin/bash\nexec -a claude /bin/bash -c '{said}; sleep 1; echo ready; read -r line' stand-in \"$@\"\n"
        ),
    };
    std::fs::write(bin.join("claude"), script).unwrap();
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The person's zsh in `home`, its prompt `ready> `, with `bin` first on its `PATH`; none
/// where there is no zsh.
pub(in crate::desk) fn shell_vars(home: &Path, bin: &Path) -> Option<Vec<(String, String)>> {
    let shell = ["/bin/zsh", "/usr/bin/zsh"]
        .into_iter()
        .find(|shell| Path::new(shell).exists())?;
    crate::terminal::keep_shell_files_in(
        std::env::temp_dir().join(format!("llxterm-test-shell-{}", std::process::id())),
    );
    std::fs::create_dir_all(home).unwrap();
    std::fs::write(home.join(".zshrc"), "PS1='ready> '\n").unwrap();
    Some(vec![
        ("SHELL".into(), shell.into()),
        ("HOME".into(), home.display().to_string()),
        ("PATH".into(), format!("{}:/usr/bin:/bin", bin.display())),
    ])
}

/// Terminal `key`'s number: `t3` is 3.
pub(in crate::desk) fn number(key: &str) -> u32 {
    key.trim_start_matches('t').parse().unwrap()
}

/// Within a while, `want` of what `read` gives; what it gave last, if never.
pub(in crate::desk) fn soon<T: std::fmt::Debug>(
    read: impl Fn() -> T,
    want: impl Fn(&T) -> bool,
) -> T {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let now = read();
        if want(&now) || Instant::now() > deadline {
            return now;
        }
        crate::backoff(Duration::from_millis(50));
    }
}

/// The agent kept for pane `key` of `desk`, once its writes are done.
pub(in crate::desk) fn kept(desk: &Desk, key: &str) -> Option<ledger::AgentSession> {
    let row = crate::lock(&desk.ledger).get(key).map(|(row, _)| *row)?;
    assert!(desk.writes_done(Duration::from_secs(5)));
    desk.feed
        .store()
        .read(|db| ledger::agent_of(db, row))
        .unwrap()
}

struct Opened {
    desk: Arc<Desk>,
    scratch: Scratch,
    vars: Vec<(String, String)>,
}

/// A person's terminal of a desk on `store`, with the stand-in on its `PATH`.
fn opened(store: Arc<crate::store::Store>, exit: Option<u8>) -> Option<(Opened, String)> {
    let scratch = Scratch::new();
    let bin: PathBuf = scratch.path().join("bin");
    stand_in(&bin, exit);
    let vars = shell_vars(&scratch.path().join("home"), &bin)?;
    let desk = desk_on(store);
    let mine = caller(&desk, scratch.path(), None);
    let call = crate::desk::Call {
        vars: Some(&vars),
        person: true,
        ..mine.call()
    };
    let group = crate::projects::key(mine.project.id);
    let key = desk.terminal_of_person(&call, &group, "", "", "").unwrap();
    drop(mine);
    Some((
        Opened {
            desk,
            scratch,
            vars,
        },
        key,
    ))
}

fn typed(desk: &Desk, key: &str, line: &str) {
    let id: u32 = number(key);
    let session = desk.session(id).unwrap();
    assert!(
        session.wait_prompt(Duration::from_secs(10)),
        "{}",
        session.snapshot()
    );
    session.type_in(line, &[]).unwrap();
}

#[test]
fn an_agent_typed_in_a_shell_is_kept_with_what_its_line_carries_and_let_go_of_as_it_ends() {
    let Some((opened, key)) = opened(Arc::new(crate::store::Store::memory().unwrap()), None) else {
        return;
    };
    let desk = &opened.desk;
    typed(
        desk,
        &key,
        &format!("claude --model opus --resume {ID} fix the cart\n"),
    );
    let agent = soon(|| kept(desk, &key), Option::is_some).expect("the agent is kept");
    assert_eq!(agent.agent, "claude");
    assert_eq!(agent.session.as_deref(), Some(ID));
    assert_eq!(agent.argv, ["--model", "opus"]);
    // The agent ends of itself: the shell is in front again, and nothing is to be resumed.
    typed_line(desk, &key, "\n");
    assert_eq!(soon(|| kept(desk, &key), Option::is_none), None);
    drop(opened.scratch);
    let _ = opened.vars;
}

fn typed_line(desk: &Desk, key: &str, line: &str) {
    let id: u32 = number(key);
    desk.session(id).unwrap().type_in(line, &[]).unwrap();
}

#[test]
fn the_session_its_hooks_name_is_kept_and_the_agent_stays_kept_as_the_app_quits() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let Some((opened, key)) = opened(Arc::clone(&store), None) else {
        return;
    };
    let desk = &opened.desk;
    typed(desk, &key, "claude --dangerously-skip-permissions\n");
    let agent = soon(|| kept(desk, &key), Option::is_some).expect("the agent is kept");
    assert_eq!(agent.session, None);
    assert_eq!(agent.argv, ["--dangerously-skip-permissions"]);
    // Its plugin says the session, from the terminal it runs in.
    let mine = caller(desk, opened.scratch.path(), None);
    let call = crate::desk::Call {
        from: Some(&key),
        ..mine.call()
    };
    let said = json_call(
        desk,
        &call,
        &serde_json::json!({"action": "interact", "session": ID}),
    );
    assert!(said.is_ok(), "{said:?}");
    let refused = json_call(
        desk,
        &call,
        &serde_json::json!({"action": "interact", "session": "x y"}),
    );
    assert!(refused.is_err());
    assert_eq!(kept(desk, &key).unwrap().session.as_deref(), Some(ID));
    // A call from no terminal names no agent's session.
    let nowhere = json_call(
        desk,
        &mine.call(),
        &serde_json::json!({"action": "interact", "session": ID}),
    );
    assert!(nowhere.is_err());
    let row = crate::lock(&desk.ledger).get(&key).unwrap().0;
    desk.shutdown();
    let after = store.read(|db| ledger::agent_of(db, row)).unwrap();
    assert_eq!(after.and_then(|agent| agent.session).as_deref(), Some(ID));
}

fn json_call(
    desk: &Desk,
    call: &crate::desk::Call,
    args: &serde_json::Value,
) -> Result<String, String> {
    desk.window(call, args)
        .map_err(|failure| failure.message().to_string())
}

#[test]
fn a_run_that_prints_and_ends_is_not_kept() {
    let Some((opened, key)) = opened(Arc::new(crate::store::Store::memory().unwrap()), None) else {
        return;
    };
    let desk = &opened.desk;
    typed(desk, &key, "claude -p hello\n");
    let id: u32 = number(&key);
    let session = desk.session(id).unwrap();
    soon(
        || session.text(),
        |text| text.contains("stand-in claude -p hello"),
    );
    crate::backoff(Duration::from_millis(300));
    assert_eq!(kept(desk, &key), None);
}

#[test]
fn an_agent_seen_and_its_session_are_in_the_store_at_once_whatever_waits_to_be_written() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let Some((opened, key)) = opened(Arc::clone(&store), None) else {
        return;
    };
    let desk = &opened.desk;
    // The desk's writer is held up, as a crash would leave what waits for it unwritten.
    let (free, held) = std::sync::mpsc::sync_channel::<()>(1);
    desk.write_later(move || {
        let _ = held.recv_timeout(Duration::from_secs(30));
    });
    let row = crate::lock(&desk.ledger).get(&key).unwrap().0;
    let stored = || store.read(|db| ledger::agent_of(db, row)).unwrap();
    typed(desk, &key, "claude --model opus\n");
    assert!(
        soon(stored, Option::is_some).is_some(),
        "kept as it is seen"
    );
    let mine = caller(desk, opened.scratch.path(), None);
    let call = crate::desk::Call {
        from: Some(&key),
        ..mine.call()
    };
    let said = json_call(
        desk,
        &call,
        &serde_json::json!({"action": "interact", "session": ID}),
    );
    assert!(said.is_ok(), "{said:?}");
    assert_eq!(
        stored().and_then(|agent| agent.session).as_deref(),
        Some(ID)
    );
    drop(free);
}
