//! Tests of `agent_relaunch`: a coding agent kept for a terminal goes on in it as the app opens
//! again, typed into its shell once the shell is ready, or says why not in the Event Center.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::desk::restore::agent_resume::tests::{ID, kept, number, shell_vars, soon, stand_in};
use crate::desk::tests::{caller, desk_on};
use crate::ledger::{self, AgentSession};
use crate::testing::Scratch;

pub(super) fn claude(session: Option<&str>, tries: u32) -> AgentSession {
    AgentSession {
        pane: 1,
        agent: "claude".into(),
        session: session.map(str::to_string),
        argv: vec![
            "--model".into(),
            "opus".into(),
            "--dangerously-skip-permissions".into(),
        ],
        folder: None,
        bypass: false,
        tries,
    }
}

#[test]
fn what_is_done_with_an_agent_kept_follows_the_setting_the_session_and_its_tries() {
    let scratch = Scratch::new();
    let bin = scratch.path().join("bin");
    stand_in(&bin, None);
    let path = bin.as_os_str();
    let plan = decide(&claude(Some(ID), 0), true, false, path).unwrap();
    assert_eq!(plan.stage, Stage::Waiting);
    assert_eq!(
        plan.line,
        format!("claude --resume {ID} --model opus --dangerously-skip-permissions")
    );
    assert_eq!(
        plan.said.as_deref(),
        Some("Resumed Claude Code session · permissions skipped")
    );
    // Turned off in Settings, or its session already live in another terminal: nothing.
    assert_eq!(decide(&claude(Some(ID), 0), false, false, path), None);
    assert_eq!(decide(&claude(Some(ID), 0), true, true, path), None);
    // Twice it did not resume: it waits for the person's Retry, nothing is typed.
    let failed = decide(&claude(Some(ID), 2), true, false, path).unwrap();
    assert_eq!((failed.stage, failed.said), (Stage::Failed, None));
    // No session known, or the agent not installed: the person may continue its folder's last.
    let unknown = decide(&claude(None, 0), true, false, path).unwrap();
    assert_eq!(
        (unknown.stage, unknown.line.as_str()),
        (Stage::Continue, "claude --continue")
    );
    let missing = decide(&claude(Some(ID), 0), true, false, OsStr::new("/nowhere")).unwrap();
    assert_eq!(missing.stage, Stage::Continue);
}

/// A desk run on `store` that leaves terminal `key` with `agent` kept, as a run that quit.
pub(super) fn left_with(store: &Arc<crate::store::Store>, folder: &Path, agent: &AgentSession) {
    let desk = desk_on(Arc::clone(store));
    let mine = caller(&desk, folder, None);
    let open = serde_json::json!({"action": "open", "kind": "terminal", "command": "sleep 30"});
    desk.window(&mine.call(), &open).unwrap();
    let row = crate::lock(&desk.ledger).get("t1").unwrap().0;
    let agent = AgentSession {
        pane: row,
        ..agent.clone()
    };
    store.write(|tx| ledger::keep_agent(tx, &agent, 1)).unwrap();
    (store.write(|tx| ledger::agent_tried(tx, row, agent.tries))).unwrap();
    desk.shutdown();
}

#[test]
fn an_agent_left_running_goes_on_in_its_terminal_as_the_app_opens_again() {
    let (store, scratch) = (
        Arc::new(crate::store::Store::memory().unwrap()),
        Scratch::new(),
    );
    let bin = scratch.path().join("bin");
    stand_in(&bin, None);
    let Some(vars) = shell_vars(&scratch.path().join("home"), &bin) else {
        return;
    };
    left_with(&store, scratch.path(), &claude(Some(ID), 0));
    let again = desk_on(Arc::clone(&store));
    let mine = caller(&again, scratch.path(), None);
    let call = Call {
        vars: Some(&vars),
        ..mine.call()
    };
    let group = crate::projects::key(mine.project.id);
    again.reopen_project(&call, &group).unwrap();
    let key = crate::lock(&again.ledger).keys().next().cloned().unwrap();
    let session = again.session(number(&key)).unwrap();
    let text = soon(
        || session.text(),
        |text| text.contains("stand-in claude --resume"),
    );
    assert!(
        text.contains("Resumed Claude Code session · permissions skipped"),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "stand-in claude --resume {ID} --model opus --dangerously-skip-permissions"
        )),
        "{text}"
    );
    // It runs in its tab again, the same session kept.
    let agent = soon(|| kept(&again, &key), |agent| agent.is_some()).unwrap();
    assert_eq!(agent.session.as_deref(), Some(ID));
    again.shutdown();
}

#[test]
fn a_session_that_does_not_resume_is_tried_twice_then_waits_for_the_persons_retry() {
    let (store, scratch) = (
        Arc::new(crate::store::Store::memory().unwrap()),
        Scratch::new(),
    );
    let bin = scratch.path().join("bin");
    stand_in(&bin, Some(1));
    let Some(vars) = shell_vars(&scratch.path().join("home"), &bin) else {
        return;
    };
    left_with(&store, scratch.path(), &claude(Some(ID), 0));
    let again = desk_on(Arc::clone(&store));
    let mine = caller(&again, scratch.path(), None);
    let call = Call {
        vars: Some(&vars),
        ..mine.call()
    };
    let group = crate::projects::key(mine.project.id);
    again.reopen_project(&call, &group).unwrap();
    let key = crate::lock(&again.ledger).keys().next().cloned().unwrap();
    let stage = || {
        crate::lock(&again.past)
            .resumes
            .get(&key)
            .map(|resume| resume.stage)
    };
    assert_eq!(
        soon(stage, |stage| *stage == Some(Stage::Failed)),
        Some(Stage::Failed)
    );
    let session = again.session(number(&key)).unwrap();
    let typed = |text: &str| text.matches("stand-in claude --resume").count();
    assert_eq!(typed(&session.text()), 2, "{}", session.text());
    assert_eq!(kept(&again, &key).unwrap().tries, 2);
    // The Event Center says so (center/resumes.rs), with the person's Retry: it types the
    // command once more.
    again.resume_again(&key);
    let text = soon(|| session.text(), |text| typed(text) == 3);
    assert_eq!(typed(&text), 3, "{text}");
    again.shutdown();

    // The app opens again: twice it did not, so nothing is typed, and the line is there.
    let third = desk_on(Arc::clone(&store));
    let mine = caller(&third, scratch.path(), None);
    let call = Call {
        vars: Some(&vars),
        ..mine.call()
    };
    third.reopen_project(&call, &group).unwrap();
    let key = crate::lock(&third.ledger).keys().next().cloned().unwrap();
    let resume = crate::lock(&third.past)
        .resumes
        .get(&key)
        .map(|resume| resume.stage);
    assert_eq!(resume, Some(Stage::Failed));
    // Its last screen, kept, shows the tries of the run before; nothing is typed under it.
    let session = third.session(number(&key)).unwrap();
    let before = typed(&session.text());
    assert!(session.wait_prompt(Duration::from_secs(10)));
    crate::backoff(Duration::from_millis(500));
    assert_eq!(typed(&session.text()), before, "{}", session.text());
    third.shutdown();
}

#[test]
fn one_session_left_in_two_terminals_goes_on_in_one() {
    let (store, scratch) = (
        Arc::new(crate::store::Store::memory().unwrap()),
        Scratch::new(),
    );
    let bin = scratch.path().join("bin");
    stand_in(&bin, None);
    let Some(vars) = shell_vars(&scratch.path().join("home"), &bin) else {
        return;
    };
    let first = desk_on(Arc::clone(&store));
    let mine = caller(&first, scratch.path(), None);
    for _ in 0..2 {
        let open = serde_json::json!({"action": "open", "kind": "terminal", "command": "sleep 30"});
        first.window(&mine.call(), &open).unwrap();
    }
    let rows: Vec<_> = crate::lock(&first.ledger)
        .values()
        .map(|(row, _)| *row)
        .collect();
    for row in &rows {
        let agent = AgentSession {
            pane: *row,
            ..claude(Some(ID), 0)
        };
        store.write(|tx| ledger::keep_agent(tx, &agent, 1)).unwrap();
    }
    first.shutdown();
    let again = desk_on(Arc::clone(&store));
    let mine = caller(&again, scratch.path(), None);
    let call = Call {
        vars: Some(&vars),
        ..mine.call()
    };
    again
        .reopen_project(&call, &crate::projects::key(mine.project.id))
        .unwrap();
    assert_eq!(crate::lock(&again.past).resumes.len(), 1);
    // The other row is skipped for good: it is not there to be looked at again.
    assert_eq!(agent_rows(&store), 1);
    again.shutdown();
}

fn agent_rows(store: &crate::store::Store) -> u64 {
    store
        .read(|db| {
            (db.query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0)))
                .map_err(|why| why.to_string())
        })
        .unwrap()
}
