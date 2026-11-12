//! Tests of `tree`: what the windows hold of a tree follows the app's news, a tree is in sight
//! from its first window to its last, and the page's messages are read as what the person did.

use std::path::PathBuf;

use super::super::told::Leaf;
use super::*;

fn branch(path: &str, dirs: &[&str], files: &[&str]) -> Branch {
    let leaf = |dir: bool| {
        move |name: &&str| Leaf {
            name: (*name).to_string(),
            dir,
        }
    };
    Branch {
        path: path.to_string(),
        leaves: dirs
            .iter()
            .map(leaf(true))
            .chain(files.iter().map(leaf(false)))
            .collect(),
        more: 0,
    }
}

fn tree(open: &[&str], dirs: Vec<Branch>) -> Tree {
    Tree {
        root: PathBuf::from("/work/app"),
        open: open.iter().map(|path| (*path).to_string()).collect(),
        dirs,
        pick: None,
    }
}

fn whole(trees: &Trees, key: &str) -> Value {
    let script = trees.whole(key).unwrap();
    serde_json::from_str(
        script
            .strip_prefix("TR(")
            .unwrap()
            .strip_suffix(')')
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn news_is_laid_over_what_is_held_and_a_folder_no_longer_open_is_let_go_of() {
    let mut trees = Trees::default();
    assert_eq!(trees.sight(1, Some("7")), [("7".to_string(), true)]);
    assert!(trees.whole("7").is_none(), "nothing told yet");
    trees.take(
        "7",
        tree(
            &["", "src"],
            vec![
                branch("", &["src"], &["Cargo.toml", "main.rs"]),
                branch("src", &["ui"], &["lib.rs"]),
            ],
        ),
    );
    let drawn = whole(&trees, "7");
    assert_eq!(drawn["k"], "7");
    assert_eq!(drawn["name"], "app");
    assert_eq!(drawn["whole"], true);
    assert_eq!(drawn["open"], serde_json::json!(["", "src"]));
    assert_eq!(
        drawn["dirs"][0],
        serde_json::json!(
            {"p": "", "d": ["src"], "f": [["Cargo.toml", "toml"], ["main.rs", "rust"]], "m": 0}
        )
    );
    // One folder changed, another opened: only they come, and the rest stays as it was.
    trees.take(
        "7",
        tree(
            &["", "src", "src/ui"],
            vec![
                branch("src", &["ui"], &["lib.rs", "new.rs"]),
                branch("src/ui", &[], &["side.css"]),
            ],
        ),
    );
    let drawn = whole(&trees, "7");
    assert_eq!(drawn["dirs"][0]["f"][0][0], "Cargo.toml");
    assert_eq!(drawn["dirs"][1]["f"][1][0], "new.rs");
    assert_eq!(drawn["dirs"][2]["p"], "src/ui");
    // `src` shut: it and what was open under it are gone.
    trees.take("7", tree(&[""], vec![]));
    assert_eq!(whole(&trees, "7")["open"], serde_json::json!([""]));
    // Another folder altogether: nothing of the old one is drawn in it.
    let mut moved = tree(&[""], vec![branch("", &[], &["only.txt"])]);
    moved.root = PathBuf::from("/work/other");
    trees.take("7", moved);
    let drawn = whole(&trees, "7");
    assert_eq!(
        (&drawn["name"], &drawn["dirs"][0]["f"][0][0]),
        (&"other".into(), &"only.txt".into())
    );
}

#[test]
fn a_tree_is_in_sight_from_its_first_window_to_its_last_and_held_only_so_long() {
    let mut trees = Trees::default();
    assert_eq!(trees.sight(1, Some("7")), [("7".to_string(), true)]);
    assert_eq!(trees.sight(1, Some("7")), [], "the same again is no news");
    assert_eq!(
        trees.sight(2, Some("7")),
        [],
        "a second window of a tree in sight"
    );
    trees.take("7", tree(&[""], vec![branch("", &[], &["a"])]));
    assert_eq!(trees.sight(1, Some("9")), [("9".to_string(), true)]);
    assert!(trees.whole("7").is_some(), "window 2 shows it still");
    assert_eq!(trees.sight(2, None), [("7".to_string(), false)]);
    assert!(trees.whole("7").is_none(), "let go of with its last window");
    // News that comes after is of a tree nobody shows.
    trees.take("7", tree(&[""], vec![branch("", &[], &["a"])]));
    assert!(trees.whole("7").is_none());
    assert_eq!(trees.sight(1, None), [("9".to_string(), false)]);
}

#[test]
fn only_what_the_tree_lists_is_a_path_of_it() {
    let mut trees = Trees::default();
    trees.sight(1, Some("7"));
    trees.take(
        "7",
        tree(
            &["", "src"],
            vec![
                branch("", &["src"], &["main.rs"]),
                branch("src", &[], &["lib.rs"]),
            ],
        ),
    );
    let at = |path: &str| trees.listed("7", path);
    assert_eq!(at(""), Some((PathBuf::from("/work/app"), true)));
    assert_eq!(at("src"), Some((PathBuf::from("/work/app/src"), true)));
    assert_eq!(
        at("src/lib.rs"),
        Some((PathBuf::from("/work/app/src/lib.rs"), false))
    );
    for outside in [
        "..",
        "../app/main.rs",
        "src/../main.rs",
        "/etc/passwd",
        "src/ui/x.rs",
        "nope",
    ] {
        assert_eq!(at(outside), None, "{outside}");
    }
    assert_eq!(trees.listed("8", "main.rs"), None, "another project's");
}

#[test]
fn the_pages_messages_are_read_as_what_the_person_did() {
    assert_eq!(said("tree files"), Some(Said::Files));
    assert_eq!(
        said("tree open src/my folder"),
        Some(Said::Open("src/my folder"))
    );
    assert_eq!(said("tree shut src"), Some(Said::Shut("src")));
    assert_eq!(said("tree act a b.txt"), Some(Said::Act("a b.txt")));
    assert_eq!(
        said("tree menu 40 212 src/a b.rs"),
        Some(Said::Menu(40, 212, "src/a b.rs"))
    );
    assert_eq!(said("tree menu 40 212"), Some(Said::Menu(40, 212, "")));
    for other in [
        "side",
        "tree",
        "tree open",
        "tree menu x y z",
        "tree files now",
        "trees files",
    ] {
        assert_eq!(said(other), None, "{other}");
    }
}

#[test]
fn news_for_the_page_names_only_what_changed() {
    let script = news(
        "7",
        &tree(&["", "src"], vec![branch("src", &[], &["lib.rs"])]),
    );
    let drawn: Value = serde_json::from_str(
        script
            .strip_prefix("TR(")
            .unwrap()
            .strip_suffix(')')
            .unwrap(),
    )
    .unwrap();
    assert_eq!(drawn["whole"], false);
    assert_eq!(drawn["open"], serde_json::json!(["", "src"]));
    assert_eq!(drawn["dirs"].as_array().unwrap().len(), 1);
}

#[test]
fn what_the_person_does_to_an_entry_is_said_with_its_path_and_a_name_after_a_line_break() {
    assert_eq!(
        said("tree make file src/my dir\na b.rs"),
        Some(Said::Make("src/my dir", "a b.rs", false))
    );
    assert_eq!(
        said("tree make dir \ndocs"),
        Some(Said::Make("", "docs", true)),
        "in the tree's own folder"
    );
    assert_eq!(
        said("tree name src/a b.rs\nc d.rs"),
        Some(Said::Name("src/a b.rs", "c d.rs"))
    );
    assert_eq!(
        said("tree move src/a.rs\n"),
        Some(Said::Move("src/a.rs", ""))
    );
    assert_eq!(
        said("tree move src/a.rs\nmy docs"),
        Some(Said::Move("src/a.rs", "my docs"))
    );
    assert_eq!(
        said("tree trash src/a b.rs"),
        Some(Said::Trash("src/a b.rs"))
    );
    assert_eq!(
        said("tree drop 640 212 src/a b.rs"),
        Some(Said::Drop(640, 212, "src/a b.rs"))
    );
    for other in [
        "tree make file src",
        "tree make link src\na",
        "tree make file src\n",
        "tree name \nx",
        "tree name a",
        "tree move \nsrc",
        "tree trash",
        "tree drop 1 2",
        "tree drop -1 2 a",
    ] {
        assert_eq!(said(other), None, "{other:?}");
    }
}

#[test]
fn a_refusal_and_a_name_to_write_are_told_to_the_page_as_data() {
    assert_eq!(
        refused("p3", "src/a.rs", "The name “b” is already taken."),
        r#"TW({"k":"p3","p":"src/a.rs","why":"The name “b” is already taken."})"#
    );
    assert_eq!(
        writes("p3", "src", Some(true)),
        r#"TE({"k":"p3","make":"dir","p":"src"})"#
    );
    assert_eq!(
        writes("p3", "src/a.rs", None),
        r#"TE({"k":"p3","make":null,"p":"src/a.rs"})"#
    );
}
