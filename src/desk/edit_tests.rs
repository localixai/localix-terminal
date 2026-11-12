//! Tests of files edited in tabs, from what an agent is answered and what the page is told: a
//! file opens in a tab of its own, is saved as the disk wrote it, follows the disk while the
//! person did not touch it, and keeps what they left unsaved past its tab.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::tests::{caller, desk};
use super::*;
use crate::testing::Scratch;

fn posted(what: &Value) -> Work {
    Work::Posted(what.to_string())
}

/// What a frame tells the page: the JSON between its name and its end.
fn told(frame: &str, name: &str) -> Value {
    let inside = frame.strip_prefix(name).and_then(|rest| {
        rest.strip_prefix('(')
            .and_then(|rest| rest.strip_suffix(')'))
    });
    serde_json::from_str(inside.unwrap_or_else(|| panic!("{frame} is no {name}"))).unwrap()
}

#[test]
fn a_file_opens_in_a_tab_of_its_own_and_is_listed_read_and_closed() {
    let scratch = Scratch::new();
    scratch.file("src/main.rs", "fn main() {}\n");
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    let open = json!({"action": "open", "kind": "file", "path": "src/main.rs", "line": 1});
    let opened = desk.act(&me.call(), &open).unwrap();
    assert!(
        opened.report.starts_with("f1 (file): "),
        "{}",
        opened.report
    );

    // The same file again is the same tab; a path alone names a file's tab.
    let again = desk
        .act(
            &me.call(),
            &json!({"action": "open", "path": "src/main.rs"}),
        )
        .unwrap();
    assert!(again.report.starts_with("f1 (file): "), "{}", again.report);

    let listed = desk.act(&me.call(), &json!({"action": "list"})).unwrap();
    assert_eq!(listed.report, "f1 (file): src/main.rs");
    let read = desk
        .act(&me.call(), &json!({"action": "interact", "tab": "f1"}))
        .unwrap();
    assert_eq!(read.state.as_deref(), Some("fn main() {}\n"));
    let typed = json!({"action": "interact", "tab": "f1", "input": "x"});
    let refused = desk.window(&me.call(), &typed).unwrap_err();
    assert!(refused.message().contains("a file tab takes no `input`"));

    let closed = desk
        .act(&me.call(), &json!({"action": "close", "tab": "f1"}))
        .unwrap();
    // Nothing was unsaved in it, and nothing is said to be kept.
    assert_eq!(closed.report, "Closed f1.");
    let listed = desk.act(&me.call(), &json!({"action": "list"})).unwrap();
    assert_eq!(listed.report, "No tabs open.");
}

#[test]
fn only_a_file_of_the_callers_project_opens_whichever_way_its_path_is_said() {
    let (scratch, outside) = (Scratch::new(), Scratch::new());
    scratch.file("in.rs", "fn main() {}\n");
    outside.file("secret.txt", "not this project's\n");
    let far = outside.path().canonicalize().unwrap().join("secret.txt");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&far, scratch.path().join("link.txt")).unwrap();
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    let name = outside.path().file_name().unwrap().to_string_lossy();
    let mut ways = vec![far.display().to_string(), format!("../{name}/secret.txt")];
    if cfg!(unix) {
        ways.push("link.txt".into());
    }
    for way in ways {
        let open = json!({"action": "open", "kind": "file", "path": way});
        let said = desk.act(&me.call(), &open).unwrap().report;
        assert!(
            said.starts_with("not opened: ") && said.contains("only the project's files"),
            "{way}: {said}"
        );
    }
    assert!(crate::lock(&desk.edits).is_empty());

    // The other project's own agent opens it, and that tab is not this one's to read or close.
    let theirs = caller(&desk, outside.path(), None);
    let open = json!({"action": "open", "kind": "file", "path": "secret.txt"});
    let opened = desk.act(&theirs.call(), &open).unwrap().report;
    assert!(opened.starts_with("f1 (file)"), "{opened}");
    // Refused each way: an action's failure, or (a close of several) its line of the report.
    for action in ["interact", "show", "close"] {
        let not = desk.act(&me.call(), &json!({"action": action, "tab": "f1"}));
        let said = not.map_or_else(|failure| failure.message().to_string(), |done| done.report);
        assert!(
            said.contains("there is no tab f1 among yours"),
            "{action}: {said}"
        );
    }
    let listed = desk.act(&me.call(), &json!({"action": "list"})).unwrap();
    assert!(!listed.report.contains("secret.txt"), "{}", listed.report);
    assert_eq!(crate::lock(&desk.edits).len(), 1);
}

#[test]
fn what_an_editor_does_not_take_opens_no_tab_and_says_why() {
    let scratch = Scratch::new();
    scratch.file("a.png", [0x89, b'P', b'N', b'G', 0, 0, 1]);
    scratch.file("big.txt", vec![b'a'; 3 * 1024 * 1024]);
    scratch.file("dir/x", "");
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    for (path, why) in [
        ("a.png", "is not text"),
        ("big.txt", "an editor takes text up to 2 MB"),
        ("dir", "is a folder"),
        ("none.txt", "there is no file"),
    ] {
        let open = json!({"action": "open", "kind": "file", "path": path});
        let said = desk.act(&me.call(), &open).unwrap().report;
        assert!(
            said.starts_with("not opened: ") && said.contains(why),
            "{said}"
        );
    }
    let no_path = desk
        .act(&me.call(), &json!({"action": "open", "kind": "file"}))
        .unwrap();
    assert!(no_path.report.contains("`path`"), "{}", no_path.report);
}

#[test]
fn the_page_is_told_the_file_as_it_loads_and_a_save_writes_it_as_the_disk_had_it() {
    let scratch = Scratch::new();
    let file = scratch.file("a.txt", "one\r\ntwo\r\n");
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    let key =
        (desk.open_file(&me.call(), &file, Some(2))).unwrap_or_else(|no| panic!("{}", no.why));

    let loaded = desk.file_work(&key, Work::Loaded);
    let first = told(&loaded[0], "O");
    assert_eq!(first["name"], "a.txt");
    assert_eq!(first["text"], "one\ntwo\n");
    assert_eq!(first["draft"], Value::Null);
    assert_eq!(first["line"], 2);
    // The line is gone to once: a page that loads again stays where it is.
    assert_eq!(
        told(&desk.file_work(&key, Work::Loaded)[0], "O")["line"],
        Value::Null
    );

    let saved = desk.file_work(
        &key,
        posted(&json!({"t": "save", "text": "one\ntwo\nthree\n"})),
    );
    assert_eq!(saved, [r#"S({"ok":true})"#]);
    assert_eq!(std::fs::read(&file).unwrap(), b"one\r\ntwo\r\nthree\r\n");
    // Its own save is no news of the disk.
    assert_eq!(desk.file_work(&key, Work::Disk), Vec::<String>::new());
    assert_eq!(desk.file_text(&key).unwrap(), "one\ntwo\nthree\n");
}

#[test]
fn a_save_over_anothers_write_writes_nothing_and_tells_what_the_disk_holds() {
    let scratch = Scratch::new();
    let file = scratch.file("a.txt", "one\n");
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    let key = (desk.open_file(&me.call(), &file, None)).unwrap_or_else(|no| panic!("{}", no.why));
    std::fs::write(&file, "another hand\n").unwrap();

    let mine = posted(&json!({"t": "save", "text": "mine\n"}));
    let answered = desk.file_work(&key, mine);
    assert_eq!(answered.len(), 2);
    assert_eq!(told(&answered[0], "D")["text"], "another hand\n");
    assert_eq!(
        told(&answered[1], "S"),
        json!({"ok": false, "conflict": true})
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "another hand\n");

    // Told what the disk holds, the person keeps theirs and saves again: it is written.
    let again = desk.file_work(&key, posted(&json!({"t": "save", "text": "mine\n"})));
    assert_eq!(again, [r#"S({"ok":true})"#]);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "mine\n");
}

#[test]
fn the_disk_changing_or_the_file_going_is_told_and_a_save_makes_it_again() {
    let scratch = Scratch::new();
    let file = scratch.file("a.txt", "one\n");
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    let key = (desk.open_file(&me.call(), &file, None)).unwrap_or_else(|no| panic!("{}", no.why));

    // The system's own notice reaches the tab: no one asks.
    std::fs::write(&file, "two\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while desk.file_text(&key).unwrap() != "two\n" {
        assert!(Instant::now() < deadline, "the change was not told");
        crate::backoff(Duration::from_millis(5));
    }

    std::fs::remove_file(&file).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !desk.file_gone(&key) {
        assert!(Instant::now() < deadline, "the file's going was not told");
        crate::backoff(Duration::from_millis(5));
    }
    let saved = desk.file_work(&key, posted(&json!({"t": "save", "text": "back\n"})));
    assert_eq!(saved, [r#"S({"ok":true})"#]);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "back\n");
}

#[test]
fn what_is_left_unsaved_outlives_its_tab_and_knows_the_disk_moved() {
    let scratch = Scratch::new();
    let file = scratch.file("a.txt", "one\n");
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    let open =
        || (desk.open_file(&me.call(), &file, None)).unwrap_or_else(|no| panic!("{}", no.why));

    let key = open();
    desk.file_work(&key, posted(&json!({"t": "draft", "text": "one\nmine\n"})));
    let listed = desk.act(&me.call(), &json!({"action": "list"})).unwrap();
    assert_eq!(listed.report, format!("{key} (file): a.txt (unsaved)"));
    assert_eq!(desk.file_text(&key).unwrap(), "one\nmine\n");
    assert_eq!(
        desk.close_file(&key).unwrap(),
        format!("Closed {key}; what was unsaved in it is kept with the file.")
    );

    let key = open();
    let loaded = told(&desk.file_work(&key, Work::Loaded)[0], "O");
    assert_eq!(loaded["text"], "one\n");
    assert_eq!(loaded["draft"], "one\nmine\n");
    assert_eq!(loaded["moved"], false);
    desk.close_file(&key).unwrap();

    // Another hand writes the file while its tab is closed.
    std::fs::write(&file, "theirs\n").unwrap();
    let key = open();
    let loaded = told(&desk.file_work(&key, Work::Loaded)[0], "O");
    assert_eq!(loaded["text"], "theirs\n");
    assert_eq!(loaded["draft"], "one\nmine\n");
    assert_eq!(loaded["moved"], true);

    // The text made the disk's again: nothing is unsaved.
    desk.file_work(&key, posted(&json!({"t": "clean"})));
    desk.close_file(&key).unwrap();
    let key = open();
    assert_eq!(
        told(&desk.file_work(&key, Work::Loaded)[0], "O")["draft"],
        Value::Null
    );
}

#[test]
fn a_file_left_open_in_two_rows_comes_back_as_one_tab_and_a_restore_under_way_is_not_run_twice() {
    use crate::ledger::{self, Kind, Opened};
    let scratch = Scratch::new();
    scratch.file("a.rs", "fn main() {}\n");
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let desk = super::super::tests::desk_on(store);
    let me = caller(&desk, scratch.path(), None);
    let path = scratch.path().canonicalize().unwrap().join("a.rs");
    // What a run before left: the same file open in two rows.
    let opened = Opened {
        project: me.project.id,
        kind: Kind::File,
        agent: None,
        title: "a.rs",
        url: None,
        cwd: Some(&path),
        command: None,
    };
    (desk.feed)
        .write(|write| {
            ledger::open(write.tx(), &opened, 1)?;
            ledger::open(write.tx(), &opened, 2).map(drop)
        })
        .unwrap();
    let person = Call {
        person: true,
        ..me.call()
    };
    let group = crate::projects::key(me.project.id);

    // A restore of the project under way opens them: a second one asked meanwhile opens nothing.
    crate::lock(&desk.past).restoring.insert(me.project.id);
    desk.reopen_project(&person, &group).unwrap();
    assert!(crate::lock(&desk.edits).is_empty());
    crate::lock(&desk.past).restoring.clear();

    desk.reopen_project(&person, &group).unwrap();
    assert_eq!(crate::lock(&desk.edits).len(), 1);
    let left = (desk.feed.store())
        .read(|db| ledger::left_open(db, me.project.id))
        .unwrap();
    assert_eq!(left.len(), 1, "{left:?}");
    let kept = crate::lock(&desk.ledger)["f1"].0;
    assert_eq!(left[0].row, kept);
}

#[test]
fn a_picked_file_an_editor_does_not_take_is_shown_as_a_page_or_by_the_persons_own_app() {
    use super::super::edit_disk::{Instead, instead};

    let binary = |name: &str| {
        instead(
            Path::new(name),
            &not_opened(Path::new(name), &Unread::Binary),
        )
    };
    // A picture or a document a page shows as it is.
    assert_eq!(binary("/work/app/logo.PNG"), Some(Instead::Page));
    assert_eq!(binary("/work/app/paper.pdf"), Some(Instead::Page));
    // Anything else there is: the app the person has for its kind.
    assert_eq!(binary("/work/app/build.zip"), Some(Instead::Outside));
    assert_eq!(binary("/work/app/tool"), Some(Instead::Outside));
    let large = not_opened(Path::new("/work/app/all.log"), &Unread::Large(64 << 20));
    assert_eq!(
        instead(Path::new("/work/app/all.log"), &large),
        Some(Instead::Outside)
    );
    // What is not there, or is no file, opens nowhere.
    let gone = not_opened(Path::new("/work/app/none.rs"), &Unread::Gone);
    assert_eq!(instead(Path::new("/work/app/none.rs"), &gone), None);
}
