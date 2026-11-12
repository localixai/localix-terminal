//! Tests of `catalog::category`: a source's own category put in one of the fixed list, and a
//! listing that names none placed by its words.

use super::*;

#[test]
fn the_list_is_fixed_and_other_is_last() {
    assert_eq!(ALL.len(), 11);
    assert_eq!(ALL[ALL.len() - 1], "Other");
}

#[test]
fn a_category_the_source_gives_is_mapped_through_the_table_whatever_its_case() {
    for (given, mapped) in [
        ("development", "Development"),
        ("Developer Tools", "Development"),
        ("developer-tools", "Development"),
        ("testing", "Development"),
        ("migration", "Development"),
        ("database", "Data"),
        ("math", "Data"),
        ("finance", "Data"),
        ("Data & Analytics", "Data"),
        ("writing", "Content"),
        ("documentation", "Content"),
        ("automation", "Productivity"),
        ("Communication", "Productivity"),
        ("Business & Operations", "Productivity"),
        ("design", "Design"),
        ("Creativity", "Design"),
        ("Security", "Security"),
        ("monitoring", "Operations"),
        ("deployment", "Operations"),
        ("learning", "Learning"),
        ("Education & Research", "Learning"),
        ("Scientific Research", "Learning"),
        ("game", "Games"),
        ("location", "Utilities"),
        ("tools", "Utilities"),
        ("  Productivity ", "Productivity"),
    ] {
        assert_eq!(of(given, "x", ""), mapped, "{given}");
    }
}

#[test]
fn a_category_that_maps_to_nothing_is_placed_by_the_words_as_none_is() {
    assert_eq!(of("misc", "pg-tools", "Query Postgres with SQL"), "Data");
    assert_eq!(of("", "pg-tools", "Query Postgres with SQL"), "Data");
}

#[test]
fn a_listing_with_no_category_is_placed_by_whole_words_of_its_name_and_summary() {
    assert_eq!(
        of("", "k8s-helper", "Deploy to Kubernetes clusters"),
        "Operations"
    );
    assert_eq!(
        of("", "secret-scan", "Find leaked secrets and CVE issues"),
        "Security"
    );
    assert_eq!(of("", "figma", "Turn designs into code"), "Design");
    assert_eq!(of("", "chess", "Play a game of chess"), "Games");
    // Only whole words: "sqlish" and "designer" are no keyword.
    assert_eq!(of("", "sqlish", "A designer's friend"), "Other");
    assert_eq!(of("", "", ""), "Other");
}

#[test]
fn the_most_matching_words_win_and_the_more_specific_category_on_a_tie() {
    // Two words of Development, one of Data.
    assert_eq!(
        of("", "repo", "Review code on GitHub, with SQL"),
        "Development"
    );
    // One each: Security comes before Development.
    assert_eq!(of("", "x", "Audit the code"), "Security");
    // One each: Data before Content.
    assert_eq!(of("", "x", "Export a csv as markdown"), "Data");
}

#[test]
fn the_keyword_table_is_lowercase_and_each_word_is_of_one_category() {
    let mut seen = std::collections::BTreeSet::new();
    for (category, words) in KEYWORDS {
        assert!(ALL.contains(&category), "{category}");
        for word in words {
            assert_eq!(*word, word.to_lowercase(), "{word}");
            assert!(word.len() <= LONGEST, "{word}");
            assert!(seen.insert(*word), "{word} is twice");
        }
        assert!((8..=15).contains(&words.len()), "{category}");
    }
    for (_, mapped) in GIVEN {
        assert!(ALL.contains(&mapped), "{mapped}");
    }
}

#[test]
fn placing_fifty_thousand_listings_is_quick() {
    let summary = "Connect your agent to the Linear issue tracker, read and write tickets, \
        search projects and cycles, and keep notes of what the team decided this week";
    let started = std::time::Instant::now();
    for n in 0..50_000 {
        let name = format!("tool-{n}");
        std::hint::black_box(of("", &name, summary));
    }
    let took = started.elapsed();
    eprintln!("category: 50k placed in {took:?}");
    assert!(took.as_secs() < 5, "{took:?}");
}
