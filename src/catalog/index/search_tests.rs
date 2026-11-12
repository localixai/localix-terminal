//! Tests of `catalog::index::search`: what is found and in what order, whatever is typed, and
//! how fast on an index as large as the registry makes it.

use std::time::Instant;

use crate::catalog::index::tests::{batch, listing};
use crate::catalog::model::{Listing, What};
use crate::catalog::{Batch, Index, Query};

/// An index of a curated source and the registry, with listings `of` each.
fn index_of(curated: Vec<Listing>, registry: Vec<Listing>) -> Index {
    let index = Index::memory().unwrap();
    index.put("cur", 3, &batch(curated, "1"), 1).unwrap();
    index.put("reg", 1, &batch(registry, "1"), 1).unwrap();
    index
}

fn ids(index: &Index, query: &Query) -> Vec<String> {
    let found = index.search(query).unwrap();
    found
        .listings
        .into_iter()
        .map(|listing| listing.id)
        .collect()
}

fn typed(text: &str) -> Query {
    Query {
        text: text.into(),
        limit: 20,
        ..Query::default()
    }
}

#[test]
fn browsing_lists_the_most_trusted_first_each_by_title() {
    let index = index_of(
        vec![
            listing("cur", "z", "Zed", "x"),
            listing("cur", "a", "alpha", "x"),
        ],
        vec![listing("reg", "b", "Beta", "x")],
    );
    assert_eq!(ids(&index, &typed("")), ["cur/a", "cur/z", "reg/b"]);
    // Nothing typed that is a word is browsing too.
    assert_eq!(ids(&index, &typed(" -- ")), ["cur/a", "cur/z", "reg/b"]);
}

#[test]
fn a_name_typed_comes_first_then_a_title_above_a_summary_then_the_trusted() {
    let index = index_of(
        vec![
            listing("cur", "notes", "Notes", "Keep notes in github issues"),
            listing("cur", "gh", "GitHub Tools", "Pull requests"),
        ],
        vec![
            listing("reg", "github", "GitHub", "The GitHub server"),
            listing("reg", "gh2", "GitHub Tools", "Pull requests"),
        ],
    );
    let found = ids(&index, &typed("github"));
    assert_eq!(found[0], "reg/github");
    // Equal matches: the trusted source's first; a summary's match last.
    assert_eq!(found[1..], ["cur/gh", "reg/gh2", "cur/notes"]);
}

#[test]
fn every_word_is_the_start_of_one_and_all_are_needed() {
    let index = index_of(
        vec![
            listing("cur", "pdf", "PDF", "Fill forms in documents"),
            listing("cur", "docx", "Word", "Edit documents"),
        ],
        vec![],
    );
    assert_eq!(ids(&index, &typed("docu")), ["cur/docx", "cur/pdf"]);
    assert_eq!(ids(&index, &typed("fill docu")), ["cur/pdf"]);
    assert_eq!(ids(&index, &typed("FORMS")), ["cur/pdf"]);
    assert!(ids(&index, &typed("spreadsheet")).is_empty());
}

#[test]
fn what_a_person_types_is_never_read_as_query_syntax() {
    let index = index_of(vec![listing("cur", "c-plus", "C++ tools", "x")], vec![]);
    for text in [
        "\"", "c\"", "-c", "c*", "title:c", "c OR", "AND", "NOT x", "(", "c)", "^c", "NEAR(c", "{",
        "'", "c++",
    ] {
        let query = typed(text);
        assert!(index.search(&query).is_ok(), "{text}");
    }
    assert_eq!(ids(&index, &typed("c++")), ["cur/c-plus"]);
    assert_eq!(ids(&index, &typed("title:c")), Vec::<String>::new());
}

#[test]
fn a_search_is_of_one_kind_when_asked_and_paged_with_its_total() {
    let mut server = listing("reg", "pdf-server", "PDF Server", "x");
    server.what = What::Server;
    let index = index_of(
        (0..5)
            .map(|n| listing("cur", &format!("pdf{n}"), &format!("PDF {n}"), "x"))
            .collect(),
        vec![server],
    );
    let only = Query {
        what: Some(What::Server),
        ..typed("pdf")
    };
    assert_eq!(ids(&index, &only), ["reg/pdf-server"]);
    let browse = Query {
        what: Some(What::Server),
        ..typed("")
    };
    assert_eq!(index.search(&browse).unwrap().total, 1);
    let page = Query {
        limit: 2,
        offset: 2,
        ..typed("pdf")
    };
    let found = index.search(&page).unwrap();
    assert_eq!(found.total, 6);
    assert_eq!(found.listings.len(), 2);
    let all: Vec<String> = ids(
        &index,
        &Query {
            limit: 10,
            ..typed("pdf")
        },
    );
    assert_eq!(found.listings[0].id, all[2]);
}

#[test]
fn browsing_puts_what_says_what_it_does_before_what_does_not() {
    let index = index_of(
        vec![
            listing("cur", "a", "Alpha", ""),
            listing("cur", "b", "Beta", "Does b"),
        ],
        vec![listing("reg", "c", "Gamma", "Does c")],
    );
    assert_eq!(ids(&index, &typed("")), ["cur/b", "cur/a", "reg/c"]);
}

#[test]
fn a_search_or_a_browse_keeps_to_the_category_and_the_source_asked_for() {
    let mut data = listing("cur", "pg", "Postgres", "Query postgres");
    data.category = "database".into();
    let mut tools = listing("cur", "git", "Git", "Query git history");
    tools.category = "development".into();
    let mut served = listing("reg", "pg-server", "Postgres Server", "Query postgres");
    served.category = "database".into();
    let index = index_of(vec![data, tools], vec![served]);
    let asked = |text: &str, category: Option<&str>, source: Option<&str>| {
        ids(
            &index,
            &Query {
                category: category.map(str::to_string),
                source: source.map(str::to_string),
                ..typed(text)
            },
        )
    };
    assert_eq!(asked("", Some("Data"), None), ["cur/pg", "reg/pg-server"]);
    assert_eq!(asked("query", Some("Data"), Some("reg")), ["reg/pg-server"]);
    assert_eq!(asked("query", None, Some("cur")), ["cur/pg", "cur/git"]);
    assert_eq!(asked("", Some("Development"), None), ["cur/git"]);
    assert!(asked("", Some("Games"), None).is_empty());
    let total = index.search(&Query {
        category: Some("Data".into()),
        ..typed("")
    });
    assert_eq!(total.unwrap().total, 2);
}

/// A listing of `source` like the many real ones, its words varied as real names and summaries
/// are; of `what`, from `origin`.
fn generated(source: &str, n: usize, what: What, origin: String) -> Listing {
    const WORDS: [&str; 12] = [
        "github", "postgres", "slack", "files", "search", "browser", "memory", "notion", "linear",
        "docker", "weather", "maps",
    ];
    let word = |k: usize| WORDS[(n / k) % WORDS.len()];
    let name = format!("io.example{n}/{}-{}", word(1), word(7));
    let summary = match n % 9 {
        0 => String::new(),
        _ => format!("A tool for {} and {}, number {n}", word(3), word(5)),
    };
    let mut listing = listing(
        source,
        &name,
        &format!("{} {}", word(1), word(13)),
        &summary,
    );
    listing.what = what;
    listing.category = String::new();
    listing.origin = origin;
    listing
}

/// `count` plugins of `source` from the origins `o0`, `o1`…
fn plugins(source: &str, count: usize) -> Vec<Listing> {
    (0..count)
        .map(|n| generated(source, n, What::Plugin, format!("github.com/o/p{n}")))
        .collect()
}

/// How long `run` took, said and held under `most` milliseconds.
fn timed<T>(what: &str, most: u128, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let done = run();
    let took = started.elapsed();
    eprintln!("60k: {what} took {took:?}");
    assert!(took.as_millis() < most, "{what} took {took:?}");
    done
}

#[test]
fn browsing_facets_a_search_and_a_whole_marketplace_stay_fast_on_sixty_thousand() {
    let scratch = crate::testing::Scratch::new();
    let index = Index::open(&scratch.path().join("catalog.sqlite")).unwrap();
    // A fifth of them listed twice: 6,000 plugins in two marketplaces, and the registry.
    let whole = |listings| Batch {
        whole: true,
        ..batch(listings, "1")
    };
    timed("the first 6,000 plugins", 5_000, || {
        index
            .put("off", 3, &whole(plugins("off", 6_000)), 1)
            .unwrap();
    });
    index
        .put("com", 2, &whole(plugins("com", 6_000)), 1)
        .unwrap();
    let servers = (0..48_000)
        .map(|n| generated("reg", n, What::Server, String::new()))
        .collect();
    index.put("reg", 1, &batch(servers, "1"), 1).unwrap();
    assert_eq!(
        index.counts(),
        [
            (What::Plugin, 6_000),
            (What::Skill, 0),
            (What::Server, 48_000)
        ]
    );
    let page = Query {
        what: Some(What::Server),
        category: Some("Data".into()),
        offset: 2_000,
        limit: 50,
        ..Query::default()
    };
    let found = timed("a browse page of a kind and category", 500, || {
        index.search(&page).unwrap()
    });
    assert!(found.total > 2_000 && found.listings.len() == 50);
    let deep = Query {
        offset: 30_000,
        limit: 50,
        ..Query::default()
    };
    timed("a browse page 30,000 deep", 500, || {
        index.search(&deep).unwrap()
    });
    let facets = timed("the facets of servers", 500, || {
        index.facets(Some(What::Server), None)
    });
    assert!(facets.len() > 2);
    timed("the facets of all", 500, || index.facets(None, None));
    timed("the facets of a source", 500, || {
        index.facets(None, Some("reg"))
    });
    let found = timed("a search", 500, || {
        index.search(&typed("postgres")).unwrap()
    });
    assert!(found.total > 1_000);
    timed("a search of a category", 500, || {
        index
            .search(&Query {
                category: Some("Data".into()),
                ..typed("postgres slack")
            })
            .unwrap()
    });
    let community = || whole(plugins("new", 2_300));
    timed("a new whole marketplace of 2,300", 3_000, || {
        index.put("new", 2, &community(), 2).unwrap();
    });
    timed("the same 2,300 read again", 3_000, || {
        index.put("new", 2, &community(), 3).unwrap();
    });
    assert_eq!(index.counts()[0], (What::Plugin, 6_000));
    timed("a source forgotten", 3_000, || index.forget("off").unwrap());
    assert_eq!(index.counts()[0], (What::Plugin, 6_000));
}
