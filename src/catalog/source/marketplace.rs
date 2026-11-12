//! A plugin marketplace kept in a GitHub repository: one file that lists its plugins, read
//! again only when its ETag changed. The two formats an agent's program reads are one
//! source, told apart by [`PluginFormat`]: Claude Code's `.claude-plugin/marketplace.json`
//! and Codex's `.agents/plugins/marketplace.json`. A plugin whose source is a command is left
//! out, since installing it runs that command; so is one its marketplace marks not
//! available.

use std::path::Path;

use serde_json::Value;

use super::{Batch, Fetch, Fetched, Source, origin, readable, short, text_at};
use crate::catalog::model::{Listing, PluginFormat, Recipe, What};

/// A plugin marketplace kept in a GitHub repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marketplace {
    /// Its name among sources, in the ids of its listings.
    pub id: &'static str,
    pub title: &'static str,
    /// `owner/repo` on GitHub: where its file is read, and where an agent adds it from.
    pub repo: &'static str,
    pub trust: u8,
    pub format: PluginFormat,
}

impl Marketplace {
    /// The address of its file, on the repository's default branch.
    pub fn url(&self) -> String {
        let file = match self.format {
            PluginFormat::Claude => ".claude-plugin/marketplace.json",
            PluginFormat::Codex => ".agents/plugins/marketplace.json",
        };
        format!(
            "https://raw.githubusercontent.com/{}/HEAD/{file}",
            self.repo
        )
    }

    /// The plugins its file `text` lists, as its listings.
    pub(super) fn listings(&self, text: &str) -> Result<Vec<Listing>, String> {
        let file: Value = serde_json::from_str(text)
            .map_err(|why| format!("the {} marketplace is not JSON: {why}", self.title))?;
        let marketplace = text_at(&file, "name");
        if marketplace.is_empty() {
            return Err(format!(
                "the {} marketplace names no marketplace",
                self.title
            ));
        }
        let plugins = file.get("plugins").and_then(Value::as_array);
        let plugins = plugins.map(Vec::as_slice).unwrap_or_default();
        Ok((plugins.iter())
            .filter_map(|plugin| self.listing(&marketplace, plugin))
            .collect())
    }

    /// One plugin entry as a listing, unless it is none, installs by running a command, or
    /// is not available.
    fn listing(&self, marketplace: &str, plugin: &Value) -> Option<Listing> {
        let name = text_at(plugin, "name");
        let from = plugin.get("source");
        let kind = from.map_or_else(String::new, |from| text_at(from, "source"));
        let policy = plugin
            .get("policy")
            .map(|policy| text_at(policy, "installation"));
        if name.is_empty() || kind == "command" || policy.as_deref() == Some("NOT_AVAILABLE") {
            return None;
        }
        let shown = Some(text_at(plugin, "displayName")).filter(|shown| !shown.is_empty());
        let shown = shown.or_else(|| {
            let shown = text_at(plugin.get("interface")?, "displayName");
            (!shown.is_empty()).then_some(shown)
        });
        let title = shown.unwrap_or_else(|| match self.format {
            PluginFormat::Claude => name.clone(),
            PluginFormat::Codex => readable(&name),
        });
        let version = Some(text_at(plugin, "version"))
            .filter(|version| !version.is_empty())
            .unwrap_or_else(|| {
                from.map(|from| short(&text_at(from, "sha")))
                    .unwrap_or_default()
            });
        Some(Listing {
            id: format!("{}/{name}", self.id),
            what: What::Plugin,
            source: self.id.to_string(),
            title,
            summary: text_at(plugin, "description"),
            author: author(plugin),
            category: text_at(plugin, "category"),
            homepage: Some(text_at(plugin, "homepage"))
                .filter(|homepage| !homepage.is_empty())
                .unwrap_or_else(|| self.folder_page(from, &kind)),
            version,
            origin: from.map_or_else(String::new, |from| origin::of_plugin(self.repo, from)),
            recipe: Recipe::Plugin {
                marketplace: marketplace.to_string(),
                from: self.repo.to_string(),
                format: self.format,
            },
            name,
        })
    }

    /// The page of a plugin kept in a folder of the marketplace's own repository; none for
    /// one kept elsewhere.
    fn folder_page(&self, from: Option<&Value>, kind: &str) -> String {
        let path = match from {
            Some(Value::String(path)) => path.clone(),
            Some(from) if kind == "local" => text_at(from, "path"),
            Some(_) | None => return String::new(),
        };
        let path = path.trim_start_matches("./").trim_matches('/');
        match path.is_empty() || path.contains("..") {
            true => String::new(),
            false => format!("https://github.com/{}/tree/HEAD/{path}", self.repo),
        }
    }
}

/// Who a plugin is by: its author's name, written as an object or as the name alone.
fn author(plugin: &Value) -> String {
    match plugin.get("author") {
        Some(Value::String(name)) => name.trim().to_string(),
        Some(by) => text_at(by, "name"),
        None => String::new(),
    }
}

impl Source for Marketplace {
    fn id(&self) -> &'static str {
        self.id
    }

    fn title(&self) -> &'static str {
        self.title
    }

    fn trust(&self) -> u8 {
        self.trust
    }

    fn read(
        &self,
        net: &dyn Fetch,
        mark: Option<&str>,
        _cache: &Path,
    ) -> Result<Option<Batch>, String> {
        let etag = mark.filter(|mark| !mark.is_empty());
        match net.text(&self.url(), etag)? {
            Fetched::Same => Ok(None),
            Fetched::New { text, etag } => Ok(Some(Batch {
                listings: self.listings(&text)?,
                gone: vec![],
                mark: etag.unwrap_or_default(),
                whole: true,
                more: false,
            })),
        }
    }
}

#[cfg(test)]
#[path = "marketplace_tests.rs"]
mod tests;
