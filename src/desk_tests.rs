//! Tests of `desk`.

use std::path::Path;

use serde_json::{Value, json};

use super::*;
#[path = "desk/events_tests.rs"]
mod events;
use crate::testing::Scratch;

/// A caller of `desk`: an agent (`agent`; none: the project's own) of the project at `cwd`.
pub(super) struct Caller {
    pub(super) project: Project,
    pub(super) agent: Option<String>,
    pub(super) owner: String,
    pub(super) cwd: PathBuf,
}

impl Caller {
    pub(super) fn call(&self) -> Call<'_> {
        Call {
            owner: &self.owner,
            project: &self.project,
            agent: self.agent.as_deref(),
            cwd: &self.cwd,
            vars: None,
            peek: false,
            again: None,
            person: false,
            window: None,
        }
    }
}

pub(super) fn caller(desk: &Desk, cwd: &Path, agent: Option<&str>) -> Caller {
    let project = desk.project(cwd).unwrap();
    Caller {
        owner: crate::projects::owner(&project, agent),
        agent: agent.map(str::to_string),
        cwd: cwd.to_path_buf(),
        project,
    }
}

/// The file a board was written out to, once every notice has been handled.
pub(super) fn written(desk: &Desk, path: &Path) -> String {
    assert!(desk.feed.settled(std::time::Duration::from_secs(5)));
    std::fs::read_to_string(path).unwrap()
}

/// A desk with no window.
pub(super) fn desk() -> Arc<Desk> {
    desk_on(Arc::new(crate::store::Store::memory().unwrap()))
}

/// A desk with no window, keeping what it keeps in `store`: two in turn on one store are a desk
/// and the same desk after a restart.
pub(super) fn desk_on(store: Arc<crate::store::Store>) -> Arc<Desk> {
    Desk::new(
        None,
        false,
        std::env::temp_dir().join("desk-test-shots"),
        Arc::new(Quiet),
        store,
    )
}

#[test]
fn a_terminal_is_opened_typed_into_listed_and_ended_by_its_owner() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let open = json!({"action": "open", "kind": "terminal",
        "command": "sh -c 'printf \"ready> \"; read x; echo got-$x; sleep 20'", "wait_for": "ready>"});
    let text = desk.window(&me, &open).unwrap();
    assert!(
        text.contains("t1 (terminal)") && text.contains("ready>"),
        "{text}"
    );

    let typed = json!({"action": "interact", "tab": "t1", "input": "7\n", "wait_for": "got-7"});
    assert!(desk.window(&me, &typed).unwrap().contains("got-7"));
    assert!(
        desk.window(&me, &json!({"action": "list"}))
            .unwrap()
            .contains("t1 (terminal)")
    );
    assert_eq!(desk.end(&mine.owner), 1);
    assert_eq!(
        desk.window(&me, &json!({"action": "list"})).unwrap(),
        "No tabs open."
    );
}

#[test]
fn another_owner_neither_sees_nor_reaches_a_tab() {
    let (desk, scratch) = (desk(), Scratch::new());
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&caller(&desk, scratch.path(), Some("a")).call(), &open)
        .unwrap();
    let theirs = caller(&desk, scratch.path(), Some("b"));
    let other = theirs.call();
    assert_eq!(
        desk.window(&other, &json!({"action": "list"})).unwrap(),
        "No tabs open."
    );
    let refused = desk
        .window(&other, &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert!(refused.contains("among yours"), "{refused}");
    desk.shutdown();
}

#[test]
fn a_page_needs_a_window_and_a_bad_action_says_what_is_known() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let no_window = desk.window(
        &me,
        &json!({"action": "open", "kind": "browser", "url": "http://localhost:3000"}),
    );
    assert!(no_window.unwrap().contains("no window here"));
    let Err(Failure::Failed(why)) = desk.window(&me, &json!({"action": "dance"})) else {
        panic!("an unknown action was taken");
    };
    assert!(why.contains("open, interact"), "{why}");
}

#[test]
fn waiting_for_text_the_input_echoes_waits_for_the_output_not_the_echo() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let open = json!({"action": "open", "kind": "terminal", "command": "sh", "yield_ms": 500});
    desk.window(&me, &open).unwrap();
    let began = std::time::Instant::now();
    let typed = json!({"action": "interact", "tab": "t1",
        "input": "sleep 1; echo zz-done\n", "wait_for": "zz-done"});
    let text = desk.window(&me, &typed).unwrap();
    assert!(
        began.elapsed() >= std::time::Duration::from_millis(900),
        "it returned on the echo: {text}"
    );
    assert!(text.contains("\"zz-done\" showed"), "{text}");
    assert_eq!(text.matches("zz-done").count(), 3, "{text}");
    assert_eq!(desk.end(&mine.owner), 1);
}

#[test]
fn a_field_a_tab_does_not_take_is_refused_with_what_it_takes_and_nothing_is_done() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let open = json!({"action": "open", "kind": "terminal",
        "command": "sh -c 'printf \"ready> \"; while read x; do echo got-$x; done'", "wait_for": "ready>"});
    desk.window(&me, &open).unwrap();

    // A page's or a board's ways on a terminal: refused, and named.
    let clicked =
        json!({"action": "interact", "tab": "t1", "op": "click", "target": "Save", "input": "7\n"});
    let refused = desk
        .window(&me, &clicked)
        .unwrap_err()
        .message()
        .to_string();
    assert!(
        refused.contains("terminal tab takes no `op`, `target`")
            && refused.contains("`input`")
            && refused.contains("nothing was done"),
        "{refused}"
    );
    // Nothing of the call was done: the input it carried was not typed.
    let read = desk
        .window(&me, &json!({"action": "interact", "tab": "t1"}))
        .unwrap();
    assert!(!read.contains("got-7"), "{read}");

    // An `op` naming a field the call carries says what it does already.
    let named = json!({"action": "interact", "tab": "t1", "op": "input", "input": "9\n", "wait_for": "got-9"});
    assert!(desk.window(&me, &named).unwrap().contains("got-9"));
    let read = desk
        .window(&me, &json!({"action": "interact", "tab": "t1"}))
        .unwrap();
    assert!(!read.contains("got-7"), "{read}");

    // Who acts is any tab's: a plugin may mark every call with it.
    let marked = json!({"action": "interact", "tab": "t1", "input": "8\n", "wait_for": "got-8", "who": "tests"});
    assert!(desk.window(&me, &marked).unwrap().contains("got-8"));

    // A board takes its own fields and no terminal's.
    desk.window(&me, &json!({"action": "open", "kind": "board"}))
        .unwrap();
    let typed = json!({"action": "interact", "tab": "b1", "keys": ["Enter"]});
    let refused = desk.window(&me, &typed).unwrap_err().message().to_string();
    assert!(
        refused.contains("board tab takes no `keys`") && refused.contains("`op`"),
        "{refused}"
    );
    let added = json!({"action": "interact", "tab": "b1", "op": "add", "title": "Fix it", "note": "Done when fixed."});
    assert!(desk.window(&me, &added).unwrap().contains("added c1"));
    desk.end(&mine.owner);
}

#[test]
fn terminals_whose_program_ended_make_room_for_the_next_one() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let call = Call {
        person: true,
        ..mine.call()
    };
    let group = crate::projects::key(mine.project.id);
    for _ in 0..term::MAX_SESSIONS {
        desk.person_terminal(&call, &group, "sh -c 'exit 0'")
            .unwrap();
    }
    // Each program ends; waiting for text that never shows returns when it does.
    let sessions: Vec<_> = crate::lock(&desk.sessions).values().cloned().collect();
    for session in sessions {
        let until = session.wait(Some("never shown"), 0, std::time::Duration::from_secs(10));
        assert_eq!(until, crate::terminal::Until::Exited);
    }
    // Their rows closed as they ended: the cap still frees the oldest for a new one.
    desk.person_terminal(&call, &group, "sh -c 'exit 0'")
        .unwrap();
    assert!(crate::lock(&desk.sessions).len() <= term::MAX_SESSIONS);
    desk.shutdown();
}

#[test]
fn ending_an_agents_session_leaves_the_tabs_the_person_opened() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200});
    let theirs = Call {
        person: true,
        ..mine.call()
    };
    desk.window(&mine.call(), &open).unwrap();
    desk.window(&theirs, &open).unwrap();
    assert_eq!(desk.end(&mine.owner), 1);
    assert!(desk.session(2).is_ok(), "the person's shell was ended");
    assert!(desk.session(1).is_err());
    desk.shutdown();
}

#[test]
fn an_ask_that_rode_on_an_act_that_failed_is_taken_back() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200});
    desk.window(&me, &open).unwrap();
    let keys: Vec<&str> = vec!["Enter"; 65];
    let failed = desk.window(
        &me,
        &json!({"action": "interact", "tab": "t1", "ask": "sign in", "keys": keys}),
    );
    assert!(failed.is_err());
    assert!(crate::lock(&desk.asks).is_empty());
    let asked = json!({"action": "interact", "tab": "t1", "ask": "sign in"});
    desk.window(&me, &asked).unwrap();
    assert_eq!(crate::lock(&desk.asks).len(), 1);
    desk.shutdown();
}

#[test]
fn open_refuses_what_it_would_drop_and_more_tabs_than_there_are_places() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), None);
    let me = mine.call();
    let why = |args: Value| desk.window(&me, &args).unwrap_err().message().to_string();
    // A typo inside `tabs`, a tab that is no object.
    let typo = why(json!({"action": "open", "tabs": [{"kind": "terminal", "comand": "sh"}]}));
    assert!(
        typo.contains("`comand`") && typo.contains("`command`"),
        "{typo}"
    );
    let no_object = why(json!({"action": "open", "tabs": ["w1"]}));
    assert!(no_object.contains("an object"), "{no_object}");
    let many = vec![json!({"kind": "board"}); 33];
    let too_many = why(json!({"action": "open", "tabs": many}));
    assert!(too_many.contains("at most 32"), "{too_many}");
    // Nothing of it was opened.
    assert_eq!(
        desk.window(&me, &json!({"action": "list"})).unwrap(),
        "No tabs open."
    );
}

#[test]
fn a_terminal_waits_at_most_what_the_schema_says() {
    let args = json!({"yield_ms": 4_000_000_000_u64});
    let default = std::time::Duration::from_secs(10);
    assert_eq!(
        args::waits(&args, default),
        std::time::Duration::from_secs(30)
    );
    assert_eq!(
        args::waits(&json!({"yield_ms": 0}), default),
        std::time::Duration::ZERO
    );
    assert_eq!(args::waits(&json!({}), default), default);
}

#[test]
fn what_a_page_wrote_loses_its_control_characters_but_keeps_its_lines() {
    assert_eq!(
        plain("a\u{1b}]52;c;eA==\u{7}b\r\nc\td"),
        "a]52;c;eA==b\nc\td"
    );
}
