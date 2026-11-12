//! Tests of `projects`.

use super::*;
use crate::store::Store;
use crate::testing::Scratch;

#[test]
fn a_project_is_the_folder_with_git_at_or_above() {
    let scratch = Scratch::new();
    let repo = scratch.path().join("shop");
    let deep = repo.join("src").join("cart");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    assert_eq!(root_of(&deep), repo);
    let loose = scratch.path().join("notes");
    std::fs::create_dir_all(&loose).unwrap();
    assert_eq!(root_of(&loose), loose);
}

#[test]
fn a_project_is_known_by_its_canonical_folder() {
    let scratch = Scratch::new();
    assert!(
        canonical(Path::new("shop"))
            .unwrap_err()
            .contains("absolute")
    );
    assert!(canonical(&scratch.path().join("none")).is_err());
    let dir = scratch.path().join("shop");
    std::fs::create_dir_all(&dir).unwrap();
    let through = scratch.path().join("shop").join("..").join("shop");
    assert_eq!(
        canonical(&through).unwrap(),
        std::fs::canonicalize(&dir).unwrap()
    );
}

#[test]
fn a_folder_is_one_project_made_once_and_only_the_person_going_to_it_opens_it() {
    let store = Store::memory().unwrap();
    let shop = Path::new("/work/shop");
    let made = store.write(|tx| find_or_make(tx, shop, 10)).unwrap();
    assert_eq!(made.name, "shop");
    let again = store.write(|tx| find_or_make(tx, shop, 20)).unwrap();
    assert_eq!(again, made);
    let other = store
        .write(|tx| find_or_make(tx, Path::new("/side/notes"), 15))
        .unwrap();
    // An agent's call finds or makes a project: none of them is used last for it.
    let before = store.read(|db| recent(db, 10)).unwrap();
    assert_eq!(before, vec![other.clone(), made.clone()]);
    store.write(|tx| opened(tx, made.id, 20)).unwrap();
    let after = store.read(|db| recent(db, 10)).unwrap();
    assert_eq!(after, vec![made, other.clone()]);
    assert_eq!(
        store.read(|db| get(db, other.id)).unwrap(),
        Some(other.clone())
    );
    store.write(|tx| forget(tx, other.id)).unwrap();
    assert_eq!(store.read(|db| get(db, other.id)).unwrap(), None);
}

#[test]
fn two_projects_of_one_name_are_told_apart_by_the_folder_above() {
    let at = |id, path: &str| Project {
        id,
        path: PathBuf::from(path),
        name: name_of(Path::new(path)),
    };
    let projects = [at(1, "/work/app"), at(2, "/side/app"), at(3, "/work/shop")];
    let labels = labels(&projects);
    assert_eq!(labels[&1], "app — work");
    assert_eq!(labels[&2], "app — side");
    assert_eq!(labels[&3], "shop");
}

#[test]
fn the_project_opened_last_is_one_whose_folder_is_still_there() {
    let scratch = Scratch::new();
    let store = Store::open(&scratch.path().join("desk.sqlite")).unwrap();
    let kept = scratch.path().join("kept");
    let gone = scratch.path().join("gone");
    std::fs::create_dir_all(&kept).unwrap();
    std::fs::create_dir_all(&gone).unwrap();
    let (kept, gone) = (canonical(&kept).unwrap(), canonical(&gone).unwrap());
    store
        .write(|tx| {
            let first = find_or_make(tx, &kept, 1)?;
            opened(tx, first.id, 10)?;
            let second = find_or_make(tx, &gone, 2)?;
            opened(tx, second.id, 20)
        })
        .unwrap();
    assert_eq!(store.read(last_there).unwrap().unwrap().path, gone);
    // Its folder deleted, the one opened before it is the last: the window still has a project
    // to open with.
    std::fs::remove_dir_all(&gone).unwrap();
    assert_eq!(store.read(last_there).unwrap().unwrap().path, kept);
    std::fs::remove_dir_all(&kept).unwrap();
    assert_eq!(store.read(last_there).unwrap(), None);
}
