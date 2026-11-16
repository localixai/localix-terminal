//! Tests of the ledger's agents: a terminal's coding agent kept as it is seen, so a crash keeps
//! it, and let go of when it ends or its terminal closes.

use std::path::Path;

use super::super::{Kind, Opened, close, forget, open};
use super::*;
use crate::store::Store;

const ID: &str = "0b8f6a52-6c1e-4b5e-9a57-3d5f0c2e9a11";

fn terminal(store: &Store) -> Row {
    let project = store
        .write(|tx| crate::projects::find_or_make(tx, Path::new("/work/shop"), 1))
        .unwrap();
    let opened = Opened {
        project: project.id,
        kind: Kind::Terminal,
        agent: None,
        title: "zsh",
        url: None,
        cwd: Some(Path::new("/work/shop")),
        command: None,
    };
    store.write(|tx| open(tx, &opened, 10)).unwrap()
}

fn claude(pane: Row, session: Option<&str>) -> AgentSession {
    AgentSession {
        pane,
        agent: "claude".into(),
        session: session.map(str::to_string),
        argv: vec!["--model".into(), "opus".into()],
        folder: Some("/work/shop/web".into()),
        bypass: false,
        tries: 0,
    }
}

#[test]
fn an_agent_is_kept_as_it_is_seen_and_its_session_once_it_is_told() {
    let store = Store::memory().unwrap();
    let pane = terminal(&store);
    assert_eq!(store.read(|db| agent_of(db, pane)).unwrap(), None);
    // Seen in front without an id: kept at once, before any quit, so a crash keeps it.
    store
        .write(|tx| keep_agent(tx, &claude(pane, None), 20))
        .unwrap();
    assert_eq!(
        store.read(|db| agent_of(db, pane)).unwrap(),
        Some(claude(pane, None))
    );
    // Its hooks say the id; the agent read again without one keeps it.
    assert!(
        store
            .write(|tx| agent_named(tx, pane, "claude", ID))
            .unwrap()
    );
    store
        .write(|tx| keep_agent(tx, &claude(pane, None), 30))
        .unwrap();
    let kept = store.read(|db| agent_of(db, pane)).unwrap().unwrap();
    assert_eq!(kept.session.as_deref(), Some(ID));
    // Another agent's word for it changes nothing.
    assert!(
        !store
            .write(|tx| agent_named(tx, pane, "codex", ID))
            .unwrap()
    );
    // Its tries are counted.
    store.write(|tx| agent_tried(tx, pane, 2)).unwrap();
    assert_eq!(
        store.read(|db| agent_of(db, pane)).unwrap().unwrap().tries,
        2
    );
}

#[test]
fn the_tries_count_afresh_for_another_session_or_another_agent() {
    let store = Store::memory().unwrap();
    let pane = terminal(&store);
    let tries = || store.read(|db| agent_of(db, pane)).unwrap().unwrap().tries;
    store
        .write(|tx| keep_agent(tx, &claude(pane, Some(ID)), 20))
        .unwrap();
    store.write(|tx| agent_tried(tx, pane, 2)).unwrap();
    // The same agent and session seen again: its tries stand.
    store
        .write(|tx| keep_agent(tx, &claude(pane, None), 25))
        .unwrap();
    assert_eq!(tries(), 2);
    // Another session in the same terminal: what the last one did not do is not its own.
    let other = "1c9e7b63-7d2f-4c6f-8b68-4e6a1d3fab22";
    store
        .write(|tx| keep_agent(tx, &claude(pane, Some(other)), 30))
        .unwrap();
    assert_eq!(tries(), 0);
    store.write(|tx| agent_tried(tx, pane, 2)).unwrap();
    // Its hooks name another session: afresh too.
    assert!(
        store
            .write(|tx| agent_named(tx, pane, "claude", ID))
            .unwrap()
    );
    assert_eq!(tries(), 0);
    store.write(|tx| agent_tried(tx, pane, 2)).unwrap();
    // Another agent: afresh.
    let codex = AgentSession {
        agent: "codex".into(),
        ..claude(pane, None)
    };
    store.write(|tx| keep_agent(tx, &codex, 40)).unwrap();
    assert_eq!(tries(), 0);
}

#[test]
fn an_agent_that_ended_or_whose_terminal_closed_is_let_go_of() {
    let store = Store::memory().unwrap();
    let pane = terminal(&store);
    store
        .write(|tx| keep_agent(tx, &claude(pane, Some(ID)), 20))
        .unwrap();
    store.write(|tx| agent_ended(tx, pane)).unwrap();
    assert_eq!(store.read(|db| agent_of(db, pane)).unwrap(), None);
    // The person closed its terminal: what ran there is never resumed.
    store
        .write(|tx| keep_agent(tx, &claude(pane, Some(ID)), 30))
        .unwrap();
    store.write(|tx| close(tx, pane, 40)).unwrap();
    assert_eq!(store.read(|db| agent_of(db, pane)).unwrap(), None);
    // A pane gone takes its agent with it.
    let pane = terminal(&store);
    store
        .write(|tx| keep_agent(tx, &claude(pane, Some(ID)), 50))
        .unwrap();
    store.write(|tx| forget(tx, pane)).unwrap();
    assert_eq!(store.read(|db| agent_of(db, pane)).unwrap(), None);
}

#[test]
fn a_row_that_does_not_read_as_words_is_no_agent() {
    let store = Store::memory().unwrap();
    let pane = terminal(&store);
    store
        .write(|tx| {
            tx.execute(
                "INSERT INTO agent_sessions (pane, agent, argv, at) VALUES (?1, 'claude', '{', 1)",
                [pane],
            )
            .map(drop)
            .map_err(|why| why.to_string())
        })
        .unwrap();
    let read = store.read(|db| agent_of(db, pane)).unwrap().unwrap();
    assert!(read.argv.is_empty());
}
