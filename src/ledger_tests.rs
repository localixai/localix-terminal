//! Tests of `ledger`: a project's tabs and the pages they showed, in the store.

use std::path::Path;

use super::*;
use crate::projects::{self, Project};
use crate::store::Store;

fn project(store: &Store, path: &str) -> Project {
    store
        .write(|tx| projects::find_or_make(tx, Path::new(path), 1))
        .unwrap()
}

fn page(project: ProjectId, url: &str) -> Opened<'_> {
    Opened {
        project,
        kind: Kind::Page,
        agent: None,
        title: "",
        url: Some(url),
        cwd: None,
        command: None,
    }
}

#[test]
fn a_tab_is_kept_open_then_closed_and_its_project_lists_it_latest_first() {
    let store = Store::memory().unwrap();
    let shop = project(&store, "/work/shop");
    let first = store
        .write(|tx| open(tx, &page(shop.id, "https://a.example/"), 10))
        .unwrap();
    let terminal = Opened {
        kind: Kind::Terminal,
        agent: Some("worker"),
        title: "npm test",
        url: None,
        cwd: Some(Path::new("/work/shop/web")),
        command: Some("npm test"),
        ..page(shop.id, "")
    };
    let second = store.write(|tx| open(tx, &terminal, 20)).unwrap();
    assert_eq!(
        store
            .read(|db| left_open(db, shop.id))
            .unwrap()
            .iter()
            .map(|tab| tab.row)
            .collect::<Vec<_>>(),
        [first, second]
    );

    store.write(|tx| close(tx, first, 30)).unwrap();
    let listed = store.read(|db| past(db, shop.id, 10)).unwrap();
    assert_eq!(listed[0].row, first);
    assert_eq!(listed[0].closed, Some(30));
    assert_eq!(listed[1].kind, Kind::Terminal);
    assert_eq!(listed[1].agent.as_deref(), Some("worker"));
    assert_eq!(listed[1].cwd.as_deref(), Some(Path::new("/work/shop/web")));
    assert_eq!(
        store.read(|db| left_open(db, shop.id)).unwrap()[0].row,
        second
    );

    // Another project's ledger is its own.
    let other = project(&store, "/work/blog");
    assert!(store.read(|db| past(db, other.id, 10)).unwrap().is_empty());
}

#[test]
fn the_pages_a_tab_showed_are_suggested_latest_first_each_once() {
    let store = Store::memory().unwrap();
    let shop = project(&store, "/work/shop");
    let row = store
        .write(|tx| open(tx, &page(shop.id, "about:blank"), 1))
        .unwrap();
    let went_to =
        |url: &str, title: &str, at: u64| store.write(|tx| went(tx, row, url, title, at)).unwrap();
    assert!(went_to("https://a.example/", "", 2));
    assert!(went_to("https://a.example/", "A", 3), "titled: news");
    assert!(!went_to("https://a.example/", "A", 4), "the same: no news");
    assert!(went_to("https://b.example/", "B", 5));
    assert!(went_to("https://a.example/", "A again", 6));
    assert!(went_to("about:blank", "", 7));
    let visits = store.read(|db| visits(db, shop.id, 10)).unwrap();
    assert_eq!(
        visits,
        [
            Visit {
                url: "https://a.example/".into(),
                title: "A again".into()
            },
            Visit {
                url: "https://b.example/".into(),
                title: "B".into()
            },
        ]
    );
    let tab = store.read(|db| get(db, row)).unwrap().unwrap();
    assert_eq!(tab.url.as_deref(), Some("about:blank"));
    assert!(store.write(|tx| titled(tx, row, "Blank")).unwrap());
    assert!(!store.write(|tx| titled(tx, row, "Blank")).unwrap());
}

#[test]
fn a_project_keeps_its_newest_closed_tabs_and_forgetting_it_takes_them() {
    let store = Store::memory().unwrap();
    let shop = project(&store, "/work/shop");
    for at in 0..(CLOSED_KEPT as u64 + 5) {
        let row = store
            .write(|tx| open(tx, &page(shop.id, "https://a.example/"), at))
            .unwrap();
        store.write(|tx| close(tx, row, at)).unwrap();
    }
    let listed = store.read(|db| past(db, shop.id, 1000)).unwrap();
    assert_eq!(listed.len(), CLOSED_KEPT);
    assert_eq!(listed[0].closed, Some(CLOSED_KEPT as u64 + 4));
    store.write(|tx| projects::forget(tx, shop.id)).unwrap();
    assert!(store.read(|db| past(db, shop.id, 10)).unwrap().is_empty());
}
