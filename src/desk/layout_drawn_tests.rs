//! Tests of what the window learns through the feed: every write that changes a project's
//! history or the projects the desk knows records its notice, which the window's consumer reads.

use std::sync::Arc;

use serde_json::json;

use super::super::tests::{caller, desk_on};
use crate::browser::Went;
use crate::testing::Scratch;

/// How many notices of `kind` the feed holds for project `project` (none: of no project).
fn told(store: &crate::store::Store, kind: &str, project: Option<i64>) -> usize {
    store
        .read(|db| {
            db.query_row(
                "SELECT COUNT(*) FROM change_events
                 WHERE json_extract(entity, '$.kind') = ?1
                   AND json_extract(entity, '$.project') IS ?2",
                rusqlite::params![kind, project],
                |row| row.get::<_, usize>(0),
            )
            .map_err(|why| why.to_string())
        })
        .unwrap()
}

#[test]
fn every_write_of_a_projects_history_or_the_projects_is_a_notice_the_window_reads() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let desk = desk_on(Arc::clone(&store));

    // A project the desk comes to know: the switcher, and its history menu.
    let mine = caller(&desk, scratch.path(), None);
    let project = Some(mine.project.id);
    let places = told(&store, "projects", None);
    assert!(places >= 1, "no notice of the projects");
    let mut past = told(&store, "ledger", project);
    assert!(past >= 1, "no notice of the project's ledger");

    // Each write of its history after it.
    let mut wrote = |what: &str, write: &dyn Fn()| {
        write();
        let now = told(&store, "ledger", project);
        assert!(now > past, "{what} did not record a notice of the ledger");
        past = now;
    };
    let open = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
        "yield_ms": 200});
    wrote("a pane opened", &|| {
        desk.window(&mine.call(), &open).unwrap();
    });
    wrote("a terminal retitled", &|| desk.retitled("t1", "building"));
    let group = crate::projects::key(mine.project.id);
    let went = Went {
        url: "https://example.com/",
        title: "Example",
        group: &group,
    };
    wrote("a page that went elsewhere", &|| desk.went("t1", &went));
    wrote("a pane closed", &|| {
        desk.window(&mine.call(), &json!({"action": "close", "tab": "t1"}))
            .unwrap();
    });

    // The person shows the project: it is the one used last, first in the switcher.
    desk.showing(&group, None);
    assert!(told(&store, "projects", None) > places);
    desk.shutdown();
}
