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
fn a_folder_is_one_project_made_once_and_opened_again() {
    let store = Store::memory().unwrap();
    let shop = Path::new("/work/shop");
    let made = store.write(|tx| find_or_make(tx, shop, 10)).unwrap();
    assert_eq!(made.name, "shop");
    let again = store.write(|tx| find_or_make(tx, shop, 20)).unwrap();
    assert_eq!(again, made);
    let other = store
        .write(|tx| find_or_make(tx, Path::new("/side/notes"), 15))
        .unwrap();
    let recent = store.read(|db| recent(db, 10)).unwrap();
    assert_eq!(recent, vec![made, other.clone()]);
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
