//! Tests of `catalog::index::shadow`: one thing two sources list is shown once, from the most
//! trusted, and shown again from the other when that one no longer lists it.

use crate::catalog::index::tests::{batch, listing};
use crate::catalog::model::{Listing, What};
use crate::catalog::{Batch, Index, Query};

/// A plugin of `source` named `name`, from `origin`.
fn plugin(source: &str, name: &str, origin: &str) -> Listing {
    let mut plugin = listing(source, name, name, "A plugin");
    plugin.what = What::Plugin;
    plugin.origin = origin.into();
    plugin
}

fn whole(listings: Vec<Listing>) -> Batch {
    Batch {
        whole: true,
        ..batch(listings, "1")
    }
}

/// The ids browsing shows.
fn shown(index: &Index) -> Vec<String> {
    let query = Query {
        limit: 50,
        ..Query::default()
    };
    let mut ids: Vec<String> = (index.search(&query).unwrap().listings.into_iter())
        .map(|listing| listing.id)
        .collect();
    ids.sort();
    ids
}

/// An index of the community's listings of `a` and `b`, and the official one's of `a`.
fn listed_twice() -> Index {
    let index = Index::memory().unwrap();
    let community = vec![
        plugin("community", "a", "github.com/o/a"),
        plugin("community", "b", "github.com/o/b"),
    ];
    index.put("community", 2, &whole(community), 1).unwrap();
    let official = vec![plugin("official", "a", "github.com/o/a")];
    index.put("official", 3, &whole(official), 1).unwrap();
    index
}

#[test]
fn the_most_trusted_listing_of_one_thing_is_shown_and_the_other_is_kept_by_its_id() {
    let index = listed_twice();
    assert_eq!(shown(&index), ["community/b", "official/a"]);
    assert!(index.get("community/a").is_some());
    let searched = Query {
        text: "a".into(),
        limit: 10,
        ..Query::default()
    };
    let found = index.search(&searched).unwrap();
    assert!(
        found
            .listings
            .iter()
            .all(|listing| listing.id != "community/a")
    );
    assert_eq!(index.counts()[0], (What::Plugin, 2));
    assert_eq!(index.also_in("official/a"), ["community"]);
    assert_eq!(index.also_in("community/a"), ["official"]);
    assert!(index.also_in("community/b").is_empty());
    assert!(index.also_in("none").is_empty());
}

#[test]
fn a_source_forgotten_shows_again_what_it_shadowed() {
    let index = listed_twice();
    index.forget("official").unwrap();
    assert_eq!(shown(&index), ["community/a", "community/b"]);
    assert_eq!(index.get("official/a"), None);
    assert_eq!(index.mark("official"), None);
    assert!(index.also_in("community/a").is_empty());
}

#[test]
fn a_listing_its_source_no_longer_has_or_moved_elsewhere_shows_the_other_again() {
    let index = listed_twice();
    index.put("official", 3, &whole(vec![]), 2).unwrap();
    assert_eq!(shown(&index), ["community/a", "community/b"]);
    let index = listed_twice();
    let moved = vec![plugin("official", "a", "github.com/o/a@v2")];
    index.put("official", 3, &whole(moved), 2).unwrap();
    assert_eq!(shown(&index), ["community/a", "community/b", "official/a"]);
    let index = listed_twice();
    let gone = Batch {
        gone: vec!["official/a".into()],
        ..batch(vec![], "2")
    };
    index.put("official", 3, &gone, 2).unwrap();
    assert_eq!(shown(&index), ["community/a", "community/b"]);
}

#[test]
fn equals_are_told_apart_by_source_then_id_and_kinds_and_unknown_origins_never_shadow() {
    let index = Index::memory().unwrap();
    let mut skill = plugin("b", "skill", "github.com/o/x");
    skill.what = What::Skill;
    let first = vec![
        plugin("b", "x2", "github.com/o/x"),
        plugin("b", "x1", "github.com/o/x"),
        skill,
        plugin("b", "free", ""),
    ];
    index.put("b", 2, &whole(first), 1).unwrap();
    assert_eq!(shown(&index), ["b/free", "b/skill", "b/x1"]);
    let other = vec![plugin("a", "x9", "github.com/o/x"), plugin("a", "free", "")];
    index.put("a", 2, &whole(other), 1).unwrap();
    assert_eq!(shown(&index), ["a/free", "a/x9", "b/free", "b/skill"]);
}
