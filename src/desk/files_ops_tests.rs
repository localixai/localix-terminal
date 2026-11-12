//! Tests of what the person does to a project's files from its tree, from what the window is
//! then told: the entry made, renamed, moved or gone at once, picked where it is now, the
//! folders left open and the tabs following it, and a refusal that changes nothing.

use serde_json::json;

use super::super::files::tests::{names, seen};
use super::super::tests::caller;
use super::*;
use crate::browser::Tree;
use crate::files::drafts;
use crate::files::ops::Refused;
use crate::testing::Scratch;

fn branch<'a>(news: &'a Tree, path: &str) -> Vec<&'a str> {
    let branch = (news.dirs.iter()).find(|branch| branch.path == path);
    names(branch.unwrap_or_else(|| panic!("{path} is not told")))
}

#[test]
fn a_file_made_in_a_folder_is_told_at_once_picked_in_its_folder_opened() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    let made = Op::Made {
        dir: "src".into(),
        name: "lib.rs".into(),
        folder: false,
    };
    assert_eq!(desk.tree_does(project, &made), Ok(()));
    assert!(scratch.path().join("src/lib.rs").is_file());
    let news = desk.tree_news(project).unwrap();
    assert_eq!(news.open, ["", "src"]);
    assert_eq!(branch(&news, "src"), ["ui", "lib.rs", "main.rs"]);
    assert_eq!(news.pick.as_deref(), Some("src/lib.rs"));
    assert_eq!(desk.tree_news(project), None, "told once");
}

#[test]
fn a_folder_renamed_stays_open_with_what_is_open_under_it() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    desk.tree_folder(project, "src", true);
    desk.tree_news(project).unwrap();
    desk.tree_folder(project, "src/ui", true);
    desk.tree_news(project).unwrap();
    let named = Op::Named {
        path: "src".into(),
        name: "source".into(),
    };
    assert_eq!(desk.tree_does(project, &named), Ok(()));
    let news = desk.tree_news(project).unwrap();
    assert_eq!(news.open, ["", "source", "source/ui"]);
    assert_eq!(branch(&news, ""), ["docs", "source", "Cargo.toml"]);
    assert_eq!(branch(&news, "source/ui"), ["side.css"]);
    assert_eq!(news.pick.as_deref(), Some("source"));
}

#[test]
fn an_entry_moved_into_a_shut_folder_opens_it_and_is_picked_there() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    let moved = Op::Moved {
        path: "Cargo.toml".into(),
        into: "docs".into(),
    };
    assert_eq!(desk.tree_does(project, &moved), Ok(()));
    let news = desk.tree_news(project).unwrap();
    assert_eq!(news.open, ["", "docs"]);
    assert_eq!(branch(&news, ""), ["docs", "src"]);
    assert_eq!(branch(&news, "docs"), ["a.md", "Cargo.toml"]);
    assert_eq!(news.pick.as_deref(), Some("docs/Cargo.toml"));
}

#[test]
fn an_entry_moved_to_the_trash_is_gone_and_its_folder_shut() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    desk.tree_folder(project, "src", true);
    desk.tree_news(project).unwrap();
    assert_eq!(desk.tree_does(project, &Op::Trashed("src".into())), Ok(()));
    assert!(!scratch.path().join("src").exists());
    let news = desk.tree_news(project).unwrap();
    assert_eq!(news.open, [""]);
    assert_eq!(branch(&news, ""), ["docs", "Cargo.toml"]);
    assert_eq!(news.pick, None);
}

#[test]
fn what_is_refused_changes_nothing_and_says_why() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    let taken = Op::Named {
        path: "docs".into(),
        name: "src".into(),
    };
    assert_eq!(
        desk.tree_does(project, &taken),
        Err(Refused::Taken("src".into()))
    );
    let out = Op::Moved {
        path: "docs".into(),
        into: "../elsewhere".into(),
    };
    assert_eq!(desk.tree_does(project, &out), Err(Refused::Gone));
    assert_eq!(desk.tree_news(project), None);
    assert!(scratch.path().join("docs/a.md").is_file());
}

#[test]
fn a_files_tab_and_what_is_unsaved_in_it_follow_the_file_renamed_or_its_folder_moved() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    let me = caller(&desk, scratch.path(), None);
    let open = json!({"action": "open", "kind": "file", "path": "src/main.rs"});
    desk.act(&me.call(), &open).unwrap();
    let root = scratch.path().canonicalize().unwrap();
    (desk.feed.store())
        .write(|tx| drafts::keep(tx, &root.join("src/main.rs"), "mine", None))
        .unwrap();
    let named = Op::Named {
        path: "src/main.rs".into(),
        name: "app.rs".into(),
    };
    assert_eq!(desk.tree_does(project, &named), Ok(()));
    let moved = Op::Moved {
        path: "src".into(),
        into: "docs".into(),
    };
    assert_eq!(desk.tree_does(project, &moved), Ok(()));
    let now = root.join("docs/src/app.rs");
    {
        let edits = crate::lock(&desk.edits);
        let kept = edits.get("f1").unwrap();
        assert_eq!(kept.path, now);
        assert_eq!(kept.shown, "docs/src/app.rs");
    }
    let draft = desk
        .feed
        .store()
        .read(|db| drafts::draft(db, &now))
        .unwrap();
    assert_eq!(draft.unwrap().text, "mine");
    let listed = desk.act(&me.call(), &json!({"action": "list"})).unwrap();
    assert_eq!(listed.report, "f1 (file): docs/src/app.rs");
}

#[test]
fn a_path_given_to_a_terminal_is_typed_as_a_shell_reads_one_name() {
    assert_eq!(typed(Path::new("/p/src/main.rs")), "/p/src/main.rs ");
    assert_eq!(
        typed(Path::new("/p/My Files/it's (1).txt")),
        "/p/My\\ Files/it\\'s\\ \\(1\\).txt "
    );
}
