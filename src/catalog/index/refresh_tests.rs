//! Tests of `catalog::index::refresh`: which sources are read, how far, and what one failing
//! leaves.

use std::path::Path;
use std::sync::Mutex;

use super::*;
use crate::catalog::index::tests::{batch, listing};
use crate::catalog::source::tests::Fake;
use crate::catalog::source::{Batch, Fetch};

/// A source that answers its pages in order, and fails where a page is an error.
struct Paged {
    id: &'static str,
    pages: Mutex<Vec<Result<Option<Batch>, String>>>,
    /// The mark it was read from each time.
    from: Mutex<Vec<Option<String>>>,
}

impl Paged {
    fn new(id: &'static str, pages: Vec<Result<Option<Batch>, String>>) -> Self {
        Self {
            id,
            pages: Mutex::new(pages),
            from: Mutex::new(vec![]),
        }
    }
}

impl Source for Paged {
    fn id(&self) -> &'static str {
        self.id
    }
    fn title(&self) -> &'static str {
        "Paged"
    }
    fn trust(&self) -> u8 {
        2
    }
    fn read(&self, _: &dyn Fetch, mark: Option<&str>, _: &Path) -> Result<Option<Batch>, String> {
        crate::lock(&self.from).push(mark.map(str::to_string));
        let mut pages = crate::lock(&self.pages);
        if pages.is_empty() {
            return Err("asked once too often".into());
        }
        pages.remove(0)
    }
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "a page among others that are errors"
)]
fn page(n: usize, mark: &str, more: bool) -> Result<Option<Batch>, String> {
    let listings = (0..n)
        .map(|k| listing("paged", &format!("{mark}-{k}"), "T", "x"))
        .collect();
    Ok(Some(Batch {
        more,
        ..batch(listings, mark)
    }))
}

#[test]
fn a_source_is_read_page_after_page_while_it_says_there_is_more() {
    let index = Index::memory().unwrap();
    let source = Paged::new("paged", vec![page(2, "p1", true), page(3, "p2", false)]);
    let heard = Mutex::new(vec![]);
    let told = |progress: &Progress| heard.lock().unwrap().push(progress.listings);
    let read = refresh(
        &index,
        &[&source],
        &Fake::default(),
        Path::new("/none"),
        10,
        false,
        &told,
    );
    assert_eq!(read, [("paged", Ok(5))]);
    assert_eq!(*source.from.lock().unwrap(), [None, Some("p1".to_string())]);
    assert_eq!(*heard.lock().unwrap(), [0, 2, 5]);
    assert_eq!(index.mark("paged"), Some(("p2".into(), 10)));
}

#[test]
fn a_source_read_within_the_hour_is_left_unless_forced_or_its_pass_was_cut_short() {
    let index = Index::memory().unwrap();
    let fresh = Paged::new("paged", vec![Ok(None), Ok(None)]);
    let net = Fake::default();
    let cache = Path::new("/none");
    index.put("paged", 2, &batch(vec![], "m"), 1000).unwrap();
    assert!(refresh(&index, &[&fresh], &net, cache, 1000 + 3599, false, &|_| {}).is_empty());
    let read = refresh(&index, &[&fresh], &net, cache, 1000 + 3600, false, &|_| {});
    assert_eq!(read, [("paged", Ok(0))]);
    // Nothing new: read at that time, its mark kept.
    assert_eq!(index.mark("paged"), Some(("m".into(), 4600)));
    assert_eq!(
        refresh(&index, &[&fresh], &net, cache, 4601, true, &|_| {}).len(),
        1
    );
    // Cut short: read at once.
    index
        .put(
            "paged",
            2,
            &Batch {
                more: true,
                ..batch(vec![], "half")
            },
            5000,
        )
        .unwrap();
    let resumed = Paged::new("paged", vec![page(1, "end", false)]);
    assert_eq!(
        refresh(&index, &[&resumed], &net, cache, 5001, false, &|_| {}).len(),
        1
    );
    assert_eq!(*resumed.from.lock().unwrap(), [Some("half".to_string())]);
}

#[test]
fn one_source_failing_leaves_what_it_wrote_and_the_others_read() {
    let index = Index::memory().unwrap();
    let failing = Paged::new(
        "failing",
        vec![page(2, "p1", true), Err("the host is down".into())],
    );
    let fine = Paged::new("fine", vec![page(1, "f", false)]);
    let read = refresh(
        &index,
        &[&failing, &fine],
        &Fake::default(),
        Path::new("/none"),
        1,
        false,
        &|_| {},
    );
    assert_eq!(
        read,
        [
            ("failing", Err("the host is down".to_string())),
            ("fine", Ok(1))
        ]
    );
    // The page it read is kept, and its pass goes on from there the next time.
    assert!(index.get("paged/p1-0").is_some());
    assert_eq!(index.mark("failing").unwrap().0, "p1");
    assert!(index.kept("failing").unwrap().more);
}
