//! Tests of the trees in sight, from what the window is told: a tree read whole as it comes
//! into sight, a folder opened or shut, a change on disk told as it happens and only for what
//! changed, and nothing read or heard of a tree no window shows.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::super::tests::{caller, desk};
use super::*;
use crate::testing::Scratch;

/// Long enough for the system to tell of a change on a busy machine.
const SOON: Duration = Duration::from_secs(10);

/// A project with a few files, its tree in sight, and the news of it read whole.
fn seen(scratch: &Scratch) -> (Arc<Desk>, ProjectId, Tree) {
    for file in ["Cargo.toml", "src/main.rs", "src/ui/side.css", "docs/a.md"] {
        scratch.file(file, "");
    }
    let desk = desk();
    let project = caller(&desk, scratch.path(), None).project.id;
    desk.tree_seen(project, true);
    let whole = desk.tree_news(project).unwrap();
    (desk, project, whole)
}

fn names(branch: &Branch) -> Vec<&str> {
    branch
        .leaves
        .iter()
        .map(|leaf| leaf.name.as_str())
        .collect()
}

/// The news of `project`'s tree once the system has told of a change.
fn news_soon(desk: &Desk, project: ProjectId) -> Tree {
    let deadline = Instant::now() + SOON;
    loop {
        if let Some(news) = desk.tree_news(project) {
            return news;
        }
        assert!(Instant::now() < deadline, "no change was told");
        crate::backoff(Duration::from_millis(5));
    }
}

/// Nothing is news of `project`'s tree for as long as a change that would be told takes.
fn quiet(desk: &Desk, project: ProjectId) {
    let deadline = Instant::now() + Duration::from_millis(400);
    while Instant::now() < deadline {
        assert_eq!(desk.tree_news(project), None);
        crate::backoff(Duration::from_millis(10));
    }
}

#[test]
fn a_tree_in_sight_is_told_whole_then_nothing_until_something_changes() {
    let scratch = Scratch::new();
    let (desk, project, whole) = seen(&scratch);
    assert_eq!(whole.root, scratch.path().canonicalize().unwrap());
    assert_eq!(whole.open, [""]);
    assert_eq!(whole.dirs.len(), 1);
    assert_eq!(names(&whole.dirs[0]), ["docs", "src", "Cargo.toml"]);
    assert!(whole.dirs[0].leaves[0].dir && !whole.dirs[0].leaves[2].dir);
    assert_eq!(desk.tree_news(project), None, "nothing changed");
}

#[test]
fn a_folder_opened_is_told_alone_kept_open_and_shut_with_what_is_under_it() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    desk.tree_folder(project, "src", true);
    let news = desk.tree_news(project).unwrap();
    assert_eq!(news.open, ["", "src"]);
    assert_eq!(news.dirs.len(), 1, "only the folder opened is read");
    assert_eq!(
        (news.dirs[0].path.as_str(), names(&news.dirs[0])),
        ("src", vec!["ui", "main.rs"])
    );
    desk.tree_folder(project, "src/ui", true);
    assert_eq!(desk.tree_news(project).unwrap().open, ["", "src", "src/ui"]);

    // Out of sight and in again (the app started again): the folders are open as they were.
    desk.tree_seen(project, false);
    assert_eq!(
        desk.tree_news(project),
        None,
        "a tree nobody shows is not read"
    );
    desk.tree_seen(project, true);
    let again = desk.tree_news(project).unwrap();
    assert_eq!(again.open, ["", "src", "src/ui"]);
    assert_eq!(again.dirs.len(), 3);

    desk.tree_folder(project, "src", false);
    let news = desk.tree_news(project).unwrap();
    assert_eq!(news.open, [""], "what is under a folder shut is shut too");
    assert!(news.dirs.is_empty(), "nothing is read to shut a folder");
}

#[test]
fn only_a_folder_the_tree_shows_can_be_opened() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    for path in [
        "..",
        "src/..",
        "/etc",
        "src/ui",
        "nowhere",
        "Cargo.toml",
        "",
    ] {
        desk.tree_folder(project, path, true);
        let news = desk.tree_news(project);
        assert!(
            news.is_none_or(|news| news.open == [""]),
            "{path} was opened"
        );
    }
}

#[test]
fn a_change_on_disk_is_told_as_it_happens_and_only_for_the_folder_it_is_in() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    desk.tree_folder(project, "src", true);
    desk.tree_news(project).unwrap();

    scratch.file("src/new.rs", "");
    let news = news_soon(&desk, project);
    assert_eq!(news.dirs.len(), 1);
    assert_eq!(
        (news.dirs[0].path.as_str(), names(&news.dirs[0])),
        ("src", vec!["ui", "main.rs", "new.rs"])
    );

    // A file only written to changes nothing the tree draws.
    std::fs::write(scratch.path().join("src/new.rs"), "fn main() {}").unwrap();
    // A change in a folder that is shut is nobody's news.
    scratch.file("docs/b.md", "");
    quiet(&desk, project);

    std::fs::remove_file(scratch.path().join("Cargo.toml")).unwrap();
    let news = news_soon(&desk, project);
    assert_eq!(
        (news.dirs[0].path.as_str(), names(&news.dirs[0])),
        ("", vec!["docs", "src"])
    );
}

#[test]
fn an_open_folder_that_goes_is_shut_and_does_not_open_again_when_it_comes_back() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    desk.tree_folder(project, "src", true);
    desk.tree_folder(project, "src/ui", true);
    desk.tree_news(project).unwrap();

    std::fs::remove_dir_all(scratch.path().join("src")).unwrap();
    let mut news = news_soon(&desk, project);
    // The system may tell of the folder's own end before its parent's.
    let deadline = Instant::now() + SOON;
    while news.open != [""] && Instant::now() < deadline {
        news = news_soon(&desk, project);
    }
    assert_eq!(news.open, [""]);

    scratch.file("src/ui/side.css", "");
    let back = news_soon(&desk, project);
    assert_eq!(back.open, [""], "it comes back shut");
    assert_eq!(names(&back.dirs[0]), ["docs", "src", "Cargo.toml"]);
}

#[test]
fn a_tree_out_of_sight_is_not_watched() {
    let scratch = Scratch::new();
    let (desk, project, _) = seen(&scratch);
    desk.tree_seen(project, false);
    scratch.file("late.txt", "");
    quiet(&desk, project);
    assert!(crate::lock(&desk.trees).is_empty());
}
