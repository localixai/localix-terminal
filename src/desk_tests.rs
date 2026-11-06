//! Tests of `desk`.

use serde_json::json;

use super::*;
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
        &json!({"action": "open", "kind": "browser", "url": "https://example.com"}),
    );
    assert!(no_window.unwrap().contains("no window here"));
    let Err(Failure::Failed(why)) = desk.window(&me, &json!({"action": "dance"})) else {
        panic!("an unknown action was taken");
    };
    assert!(why.contains("open, interact"), "{why}");
}

#[test]
fn what_the_person_sent_reaches_the_owner_of_the_tab_once() {
    let (desk, scratch) = (desk(), Scratch::new());
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    desk.window(
        &me,
        &json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300}),
    )
    .unwrap();
    crate::lock(&desk.registry).hear("t1", "this button is wrong");
    let said = desk.said(&mine.owner).unwrap();
    assert!(
        said.contains("this button is wrong") && said.contains("untrusted"),
        "{said}"
    );
    assert!(desk.said(&mine.owner).is_none());
    desk.shutdown();
}

/// Records what the desk says happened.
#[derive(Default)]
struct Told(Mutex<Vec<(String, Option<String>, Value)>>);

impl Events for Told {
    fn publish(&self, event: &str, owner: Option<&str>, params: Value) {
        crate::lock(&self.0).push((event.to_string(), owner.map(String::from), params));
    }
}

#[test]
fn a_comment_is_announced_kept_until_acknowledged_and_a_closed_tab_is_announced() {
    let told = Arc::new(Told::default());
    let events: Arc<dyn Events> = Arc::<Told>::clone(&told);
    let desk = Desk::new(
        None,
        false,
        std::env::temp_dir().join("desk-test-shots"),
        events,
        Arc::new(crate::store::Store::memory().unwrap()),
    );
    let scratch = Scratch::new();
    let mine = caller(&desk, scratch.path(), Some("me"));
    let me = mine.call();
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 20", "yield_ms": 300});
    desk.window(&me, &open).unwrap();

    desk.person_says("t1", "this button is wrong");
    let said = desk.pending(&mine.owner);
    assert_eq!(said.len(), 1);
    // The relay tells it from the feed, on a thread of its own.
    assert!(desk.feed.settled(std::time::Duration::from_secs(5)));
    {
        let told = crate::lock(&told.0);
        assert_eq!(told[0].0, "comment/added");
        assert_eq!(told[0].1.as_deref(), Some(mine.owner.as_str()));
        assert_eq!(told[0].2["agent"], "me");
        assert_eq!(
            told[0].2["project"],
            mine.project.path.display().to_string()
        );
        assert_eq!(told[0].2["text"], "this button is wrong");
        assert_eq!(told[0].2["id"], said[0].id);
    }
    // Told is not delivered: it stays until a call carries it, or it is acknowledged.
    assert_eq!(desk.pending(&mine.owner).len(), 1);
    desk.acknowledge(&mine.owner, &[said[0].id]);
    assert!(desk.pending(&mine.owner).is_empty());
    assert!(desk.said(&mine.owner).is_none());

    desk.window(&me, &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    assert!(desk.feed.settled(std::time::Duration::from_secs(5)));
    let told = crate::lock(&told.0);
    assert!(
        told.iter()
            .any(|(event, owner, params)| event == "tab/closed"
                && owner.as_deref() == Some(mine.owner.as_str())
                && params["tab"] == "t1")
    );
    drop(told);
    desk.shutdown();
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
