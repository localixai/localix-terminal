//! Tests of `catalog::source`, and the network a source's test reads through: answers given
//! in order, files copied from a folder of the test's own. No test reaches the network.

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;

use super::*;
use crate::catalog::model::PluginFormat;

/// A network that answers what it was given, in order, and keeps what it was asked.
#[derive(Default)]
pub(crate) struct Fake {
    texts: Mutex<VecDeque<Result<Fetched, String>>>,
    files: BTreeMap<String, PathBuf>,
    /// Each address asked for text, with the ETag it was asked by.
    pub(crate) asked: Mutex<Vec<(String, Option<String>)>>,
    /// Each address a file was fetched from.
    pub(crate) fetched: Mutex<Vec<String>>,
}

impl Fake {
    /// A network whose answers are `texts`, one each time it is asked.
    pub(crate) fn answering(texts: impl IntoIterator<Item = Result<Fetched, String>>) -> Self {
        Self {
            texts: Mutex::new(texts.into_iter().collect()),
            ..Self::default()
        }
    }

    /// This network, the file at `url` the one at `from`.
    pub(crate) fn with_file(mut self, url: &str, from: PathBuf) -> Self {
        self.files.insert(url.to_string(), from);
        self
    }

    pub(crate) fn asked(&self) -> Vec<(String, Option<String>)> {
        self.asked.lock().unwrap().clone()
    }
}

/// A text answer that is new, with its ETag.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an answer among others that are errors"
)]
pub(crate) fn new(text: &str, etag: Option<&str>) -> Result<Fetched, String> {
    Ok(Fetched::New {
        text: text.to_string(),
        etag: etag.map(str::to_string),
    })
}

impl Fetch for Fake {
    fn text(&self, url: &str, etag: Option<&str>) -> Result<Fetched, String> {
        let asked = (url.to_string(), etag.map(str::to_string));
        crate::lock(&self.asked).push(asked);
        let next = crate::lock(&self.texts).pop_front();
        next.unwrap_or_else(|| Err(format!("{url}: nothing more to answer")))
    }

    fn file(&self, url: &str, to: &Path) -> Result<(), String> {
        crate::lock(&self.fetched).push(url.to_string());
        let from = self
            .files
            .get(url)
            .ok_or_else(|| format!("{url}: no such file"))?;
        // As the network does: through curl, from a file:// address.
        let from = format!("file://{}", from.display());
        crate::fetch::file(&from, to, Duration::from_secs(20), &|_| {})
    }
}

#[test]
fn every_source_has_its_own_stable_id_and_a_vendors_are_trusted_above_the_rest() {
    let ids: Vec<&str> = all().iter().map(|source| source.id()).collect();
    assert_eq!(
        ids,
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
    let trusts: Vec<u8> = all().iter().map(|source| source.trust()).collect();
    assert_eq!(trusts, [3, 2, 3, 3, 3, 2, 2, 1]);
    for source in all() {
        assert!(!source.title().is_empty(), "{}", source.id());
    }
    let defaults: Vec<&str> = defaults().iter().map(|source| source.id()).collect();
    assert_eq!(defaults, ids);
}

#[test]
fn a_source_the_person_configured_is_made_from_its_words() {
    let made = marketplace(
        "acme",
        "Acme Plugins",
        "acme/plugins",
        2,
        PluginFormat::Codex,
    );
    assert_eq!(
        (made.id(), made.title(), made.trust()),
        ("acme", "Acme Plugins", 2)
    );
    let net = Fake::answering([new(r#"{"name":"acme","plugins":[{"name":"x"}]}"#, None)]);
    let batch = made
        .read(&net, None, &std::env::temp_dir())
        .unwrap()
        .unwrap();
    assert_eq!(batch.listings[0].id, "acme/x");
    assert_eq!(
        net.asked()[0].0,
        "https://raw.githubusercontent.com/acme/plugins/HEAD/.agents/plugins/marketplace.json"
    );
    let skills = skills_repo("acme-skills", "Acme Skills", "acme/skills", 2);
    assert_eq!((skills.id(), skills.trust()), ("acme-skills", 2));
}

#[test]
fn a_repository_is_owner_and_name_and_nothing_else() {
    for good in ["anthropics/skills", "a/b", "my.org/re_po-2", "a/b.c"] {
        assert!(repo_ok(good), "{good}");
    }
    let long = format!("{}/x", "a".repeat(101));
    for bad in [
        "",
        "a",
        "/b",
        "a/",
        "a/b/c",
        ".a/b",
        "a/.b",
        "a/b..c",
        "a b/c",
        "a/b?x",
        "https://github.com/a/b",
        "a/b\n",
        &long,
    ] {
        assert!(!repo_ok(bad), "{bad}");
    }
}

#[test]
fn a_name_is_read_as_words() {
    assert_eq!(readable("linear"), "Linear");
    assert_eq!(readable("google-drive"), "Google Drive");
    assert_eq!(readable("hugging_face--hub"), "Hugging Face Hub");
    assert_eq!(readable("évite"), "Évite");
    assert_eq!(readable(""), "");
}
