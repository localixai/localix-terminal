//! Tests of `agent_relaunch` with the person in it: a resume types only into the shell at a
//! clean prompt, the person's Retry and Continue are taken once and need no prompt mark, an
//! agent that ends well at once is let go of, and one the person starts takes the line away.

use std::sync::Arc;
use std::time::Duration;

use super::tests::{claude, left_with};
use super::*;
use crate::desk::restore::agent_resume::tests::{ID, kept, number, shell_vars, soon, stand_in};
use crate::desk::tests::{caller, desk_on};
use crate::ledger::AgentSession;
use crate::testing::Scratch;

/// A desk opened again on `store`, with `agent` left in its one terminal, the stand-in ending
/// with `exit`, and the shell's `extra` variables: the desk, its terminal's key, and the scratch.
fn reopened(
    agent: &AgentSession,
    exit: Option<u8>,
    extra: &[(&str, &str)],
) -> Option<(Arc<Desk>, String, Arc<crate::store::Store>, Scratch)> {
    let (store, scratch) = (
        Arc::new(crate::store::Store::memory().unwrap()),
        Scratch::new(),
    );
    let bin = scratch.path().join("bin");
    stand_in(&bin, exit);
    let mut vars = shell_vars(&scratch.path().join("home"), &bin)?;
    vars.extend(
        extra
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string())),
    );
    left_with(&store, scratch.path(), agent);
    let again = desk_on(Arc::clone(&store));
    let mine = caller(&again, scratch.path(), None);
    let call = Call {
        vars: Some(&vars),
        ..mine.call()
    };
    again
        .reopen_project(&call, &crate::projects::key(mine.project.id))
        .unwrap();
    let key = crate::lock(&again.ledger).keys().next().cloned()?;
    drop(mine);
    Some((again, key, store, scratch))
}

fn stage_of(desk: &Desk, key: &str) -> Option<Stage> {
    (crate::lock(&desk.past).resumes.get(key)).map(|resume| resume.stage)
}

fn typed_count(text: &str) -> usize {
    text.matches("stand-in claude --resume").count()
}

#[test]
fn a_retry_types_nothing_into_a_program_in_front_and_a_double_click_types_once() {
    let Some((desk, key, _store, _scratch)) = reopened(&claude(Some(ID), 2), Some(1), &[]) else {
        return;
    };
    assert_eq!(stage_of(&desk, &key), Some(Stage::Failed));
    let session = desk.session(number(&key)).unwrap();
    assert!(session.wait_prompt(Duration::from_secs(10)));
    let before = typed_count(&session.text());
    // The person runs a command of their own: a Retry now would type into it.
    session.type_in("sleep 3\n", &[]).unwrap();
    let front = soon(
        || {
            session
                .front_command()
                .and_then(|argv| argv.first().cloned())
        },
        |first| first.as_deref() == Some("sleep"),
    );
    assert_eq!(front.as_deref(), Some("sleep"));
    desk.resume_again(&key);
    assert_eq!(
        soon(
            || stage_of(&desk, &key),
            |stage| *stage == Some(Stage::Continue)
        ),
        Some(Stage::Continue)
    );
    // Its end, and the shell's prompt again: the person's two clicks start one attempt.
    let ended = soon(
        || {
            session
                .front_command()
                .and_then(|argv| argv.first().cloned())
        },
        |first| first.as_deref() != Some("sleep"),
    );
    assert_ne!(ended.as_deref(), Some("sleep"));
    crate::backoff(Duration::from_millis(300));
    desk.resume_again(&key);
    desk.resume_again(&key);
    let text = soon(|| session.text(), |text| typed_count(text) > before);
    assert_eq!(
        soon(
            || stage_of(&desk, &key),
            |stage| *stage == Some(Stage::Failed)
        ),
        Some(Stage::Failed)
    );
    crate::backoff(Duration::from_millis(500));
    assert_eq!(typed_count(&session.text()), before + 1, "{text}");
    desk.shutdown();
}

#[test]
fn a_retry_in_a_shell_that_never_marks_its_prompt_types_once_the_shell_is_in_front() {
    let unmarked = [("LLXTERM_SHELL_MARKS", "0")];
    let Some((desk, key, _store, _scratch)) = reopened(&claude(Some(ID), 2), Some(1), &unmarked)
    else {
        return;
    };
    let session = desk.session(number(&key)).unwrap();
    assert!(
        !session.wait_prompt(Duration::from_millis(300)),
        "it marks nothing"
    );
    desk.resume_again(&key);
    let text = soon(|| session.text(), |text| typed_count(text) == 1);
    assert_eq!(typed_count(&text), 1, "{text}");
    desk.shutdown();
}

#[test]
fn an_agent_that_ends_well_at_once_is_let_go_of_not_counted_as_a_try() {
    let Some((desk, key, _store, _scratch)) = reopened(&claude(Some(ID), 0), Some(0), &[]) else {
        return;
    };
    let session = desk.session(number(&key)).unwrap();
    soon(|| session.text(), |text| typed_count(text) == 1);
    assert_eq!(
        soon(|| kept(&desk, &key), Option::is_none),
        None,
        "it ended: nothing to resume"
    );
    assert_eq!(stage_of(&desk, &key), None);
    desk.shutdown();
}

#[test]
fn a_resume_waiting_for_the_person_goes_when_an_agent_comes_in_front_of_its_terminal() {
    let Some((desk, key, _store, _scratch)) = reopened(&claude(Some(ID), 2), None, &[]) else {
        return;
    };
    assert_eq!(stage_of(&desk, &key), Some(Stage::Failed));
    let session = desk.session(number(&key)).unwrap();
    assert!(session.wait_prompt(Duration::from_secs(10)));
    // The person starts the agent themselves: no Retry is left for it.
    session.type_in("claude\n", &[]).unwrap();
    assert_eq!(soon(|| stage_of(&desk, &key), Option::is_none), None);
    // It ends of itself: nothing is kept to resume.
    soon(|| session.text(), |text| text.contains("ready"));
    session.type_in("\n", &[]).unwrap();
    assert_eq!(soon(|| kept(&desk, &key), Option::is_none), None);
    desk.shutdown();
}
