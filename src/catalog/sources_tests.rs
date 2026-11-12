//! The person's list of sources: what it begins with, and that what they change is kept.

use super::*;

fn file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("llx-sources-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("sources.json")
}

fn ids(sources: &Sources) -> Vec<&str> {
    (sources.list().iter())
        .map(|kept| kept.entry.id.as_str())
        .collect()
}

#[test]
fn a_new_list_has_the_sources_the_app_trusts() {
    let sources = Sources::open(&file("new"));
    assert_eq!(
        ids(&sources),
        [
            "claude-plugins",
            "claude-community",
            "codex-plugins",
            "skills",
            "codex-skills",
            "hf-skills",
            "vercel-skills",
            "mcp-registry"
        ]
    );
}

#[test]
fn one_removed_stays_removed_until_the_person_restores_it() {
    let file = file("removed");
    let mut sources = Sources::open(&file);
    assert!(sources.remove("mcp-registry"));
    assert!(!sources.remove("mcp-registry"));

    let mut again = Sources::open(&file);
    assert!(again.get("mcp-registry").is_none());
    assert_eq!(again.removed(), 1);

    again.restore();
    assert_eq!(ids(&again).last(), Some(&"mcp-registry"));
    assert_eq!(Sources::open(&file).removed(), 0);
}

#[test]
fn a_repository_the_person_adds_is_kept() {
    let file = file("added");
    let mut sources = Sources::open(&file);
    let (shape, source) = ways("Acme/Tools").remove(2);
    assert_eq!(shape, Shape::Skills);
    sources.add("Acme/Tools", shape, source);

    let again = Sources::open(&file);
    let kept = again.get("acme-tools").map(|kept| kept.entry.clone());
    assert_eq!(
        kept,
        Some(Entry {
            id: "acme-tools".into(),
            repo: "Acme/Tools".into(),
            shape: Shape::Skills,
            stock: false,
        })
    );
    assert_eq!(
        again.get("acme-tools").map(|kept| kept.source.trust()),
        Some(2)
    );
    assert!(again.of_repo("acme/tools").is_some());
}

#[test]
fn a_file_cannot_pass_a_source_of_its_own_off_as_the_apps() {
    let file = file("forged");
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(
        &file,
        r#"{"sources": [
            {"id": "evil", "repo": "evil/list", "shape": "claude", "stock": true},
            {"id": "odd", "repo": "../../etc", "shape": "claude"},
            {"id": "reg", "repo": "a/b", "shape": "registry"}
        ]}"#,
    )
    .unwrap();
    let sources = Sources::open(&file);
    let evil = sources.get("evil").map(|kept| kept.entry.stock);
    assert_eq!(evil, Some(false));
    assert_eq!(sources.get("evil").map(|kept| kept.source.trust()), Some(2));
    assert!(sources.get("odd").is_none());
    assert!(sources.get("reg").is_none());
}

#[test]
fn a_repository_is_read_from_its_name_or_its_address() {
    for text in [
        "acme/tools",
        " acme/tools/ ",
        "https://github.com/acme/tools",
        "https://github.com/acme/tools.git",
        "github.com/acme/tools/tree/main/plugins",
        "git@github.com:acme/tools.git",
    ] {
        assert_eq!(repo_of(text).as_deref(), Some("acme/tools"), "{text}");
    }
    for text in ["", "acme", "https://example.com", "../x", "a b/c"] {
        assert_eq!(repo_of(text), None, "{text}");
    }
    assert_eq!(id_of("Acme/My.Tools"), "acme-my-tools");
}
