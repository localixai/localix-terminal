//! Tests of `catalog::index::held`: the categories a page filters by, and the sources held.

use crate::catalog::index::tests::{batch, listing};
use crate::catalog::model::{Listing, What};
use crate::catalog::{Held, Index};

/// A listing of `source` named `name`, of `what`, whose source calls its category `category`.
fn of(source: &str, name: &str, what: What, category: &str) -> Listing {
    let mut listing = listing(source, name, name, "x");
    listing.what = what;
    listing.category = category.into();
    listing
}

fn index() -> Index {
    let index = Index::memory().unwrap();
    let plugins = vec![
        of("cur", "a", What::Plugin, "development"),
        of("cur", "b", What::Plugin, "Developer Tools"),
        of("cur", "c", What::Plugin, "database"),
        of("cur", "d", What::Plugin, "nothing known"),
        of("cur", "e", What::Skill, "design"),
    ];
    index.put("cur", 3, &batch(plugins, "1"), 100).unwrap();
    let servers = vec![
        of("reg", "f", What::Server, ""),
        of("reg", "g", What::Server, "database"),
    ];
    index.put("reg", 1, &batch(servers, "1"), 200).unwrap();
    index
}

fn facets(index: &Index, what: Option<What>, source: Option<&str>) -> Vec<(String, u64)> {
    index.facets(what, source)
}

fn named(facets: &[(&str, u64)]) -> Vec<(String, u64)> {
    (facets.iter())
        .map(|(name, count)| (name.to_string(), *count))
        .collect()
}

#[test]
fn the_categories_of_what_is_shown_are_counted_biggest_first_and_other_last() {
    let index = index();
    assert_eq!(
        facets(&index, None, None),
        named(&[("Data", 2), ("Development", 2), ("Design", 1), ("Other", 2)])
    );
    assert_eq!(
        facets(&index, Some(What::Plugin), None),
        named(&[("Development", 2), ("Data", 1), ("Other", 1)])
    );
    assert_eq!(
        facets(&index, None, Some("reg")),
        named(&[("Data", 1), ("Other", 1)])
    );
    assert!(facets(&index, Some(What::Skill), Some("reg")).is_empty());
    // A listing read back carries the category it was placed in.
    assert_eq!(index.get("cur/b").unwrap().category, "Development");
}

#[test]
fn each_source_is_held_with_all_it_lists_and_when_it_was_read() {
    let index = index();
    let mut shadowed = of("dup", "a", What::Plugin, "");
    shadowed.origin = "github.com/o/a".into();
    let mut over = of("cur", "a", What::Plugin, "development");
    over.origin = "github.com/o/a".into();
    index
        .put("dup", 1, &batch(vec![shadowed], "1"), 300)
        .unwrap();
    index.put("cur", 3, &batch(vec![over], "2"), 400).unwrap();
    let held = |source: &str, listings: u64, read_at: u64| Held {
        source: source.into(),
        listings,
        read_at,
    };
    assert_eq!(
        index.sources(),
        [
            held("cur", 5, 400),
            held("dup", 1, 300),
            held("reg", 2, 200)
        ]
    );
    index.forget("reg").unwrap();
    assert_eq!(index.sources().len(), 2);
    assert_eq!(index.counts()[2], (What::Server, 0));
}
