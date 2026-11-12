//! Tests of `catalog::index`: what a source's batch writes, removes and marks.

use super::*;
use crate::catalog::model::Recipe;

/// A listing of `source` named `name`, the rest of it said by `title` and `summary`.
pub(in crate::catalog) fn listing(source: &str, name: &str, title: &str, summary: &str) -> Listing {
    Listing {
        id: format!("{source}/{name}"),
        what: What::Skill,
        source: source.into(),
        name: name.into(),
        title: title.into(),
        summary: summary.into(),
        author: "Anthropic".into(),
        category: "Content".into(),
        homepage: String::new(),
        version: "1".into(),
        origin: String::new(),
        recipe: Recipe::Skill {
            repo: "anthropics/skills".into(),
            rev: "main".into(),
            path: format!("skills/{name}"),
        },
    }
}

/// A batch of `listings` that leaves `mark`.
pub(in crate::catalog) fn batch(listings: Vec<Listing>, mark: &str) -> Batch {
    Batch {
        listings,
        mark: mark.into(),
        ..Batch::default()
    }
}

#[test]
fn a_batch_is_written_whole_with_its_mark_and_read_back_as_it_was() {
    let index = Index::memory().unwrap();
    let pdf = listing("skills", "pdf", "PDF", "Read and fill PDF forms");
    index
        .put("skills", 3, &batch(vec![pdf.clone()], "e1"), 100)
        .unwrap();
    assert_eq!(index.get("skills/pdf"), Some(pdf));
    assert_eq!(index.get("skills/none"), None);
    assert_eq!(index.mark("skills"), Some(("e1".into(), 100)));
    assert_eq!(index.mark("other"), None);
    assert_eq!(
        index.counts(),
        [(What::Plugin, 0), (What::Skill, 1), (What::Server, 0)]
    );
    // Nothing new: the mark stays, read later.
    index.touch("skills", 200).unwrap();
    assert_eq!(index.mark("skills"), Some(("e1".into(), 200)));
}

#[test]
fn a_whole_batch_replaces_its_sources_set_and_leaves_the_others() {
    let index = Index::memory().unwrap();
    let first = vec![
        listing("skills", "pdf", "PDF", "x"),
        listing("skills", "docx", "Word", "x"),
    ];
    index
        .put(
            "skills",
            3,
            &Batch {
                whole: true,
                ..batch(first, "1")
            },
            1,
        )
        .unwrap();
    index
        .put(
            "other",
            1,
            &batch(vec![listing("other", "pdf", "PDF", "x")], "1"),
            1,
        )
        .unwrap();
    let mut changed = listing("skills", "pdf", "PDF Tools", "Changed");
    changed.version = "2".into();
    index
        .put(
            "skills",
            3,
            &Batch {
                whole: true,
                ..batch(vec![changed.clone()], "2")
            },
            2,
        )
        .unwrap();
    assert_eq!(index.get("skills/docx"), None);
    assert_eq!(index.get("skills/pdf"), Some(changed));
    assert!(index.get("other/pdf").is_some());
    // The words follow: the old title is not found, the new one is.
    let found = |text: &str| {
        let query = Query {
            text: text.into(),
            limit: 10,
            ..Query::default()
        };
        let found = index.search(&query).unwrap();
        found
            .listings
            .into_iter()
            .map(|listing| listing.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(found("tools"), ["skills/pdf"]);
    assert!(found("word").is_empty());
}

#[test]
fn what_is_gone_is_removed_and_only_from_its_own_source() {
    let index = Index::memory().unwrap();
    let both = vec![listing("reg", "a", "A", "x"), listing("reg", "b", "B", "x")];
    index.put("reg", 1, &batch(both, "1"), 1).unwrap();
    index
        .put(
            "cur",
            3,
            &batch(vec![listing("cur", "a", "A", "x")], "1"),
            1,
        )
        .unwrap();
    let gone = Batch {
        gone: vec!["reg/a".into(), "cur/a".into()],
        more: true,
        ..batch(vec![], "2")
    };
    index.put("reg", 1, &gone, 2).unwrap();
    assert_eq!(index.get("reg/a"), None);
    assert!(index.get("reg/b").is_some());
    assert!(index.get("cur/a").is_some());
    assert!(index.kept("reg").unwrap().more);
}
