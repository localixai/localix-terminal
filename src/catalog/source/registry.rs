//! The official MCP Registry: tens of thousands of servers, read a page of a hundred at a time,
//! one request after another. The first pass reads them all, the passes after only what was
//! updated since (`pass.rs`); a server the registry marks deleted is gone. How each server is
//! reached is `runs.rs`.

mod pass;
mod runs;

use std::path::Path;

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;

use super::{Batch, Fetch, Fetched, Source, readable, text_at};
use crate::catalog::model::{Listing, Recipe, What};
use pass::Pass;

/// The servers, the latest version of each, a page at a time.
const SERVERS: &str =
    "https://registry.modelcontextprotocol.io/v0.1/servers?limit=100&version=latest";

/// Where the registry says what it knows of a server beside the server itself.
const OFFICIAL: &str = "io.modelcontextprotocol.registry/official";

pub struct Registry {
    /// Now, in seconds of the epoch: when a pass begins.
    pub now: fn() -> u64,
}

impl Source for Registry {
    fn id(&self) -> &'static str {
        "mcp-registry"
    }

    fn title(&self) -> &'static str {
        "MCP Registry"
    }

    fn trust(&self) -> u8 {
        1
    }

    fn read(
        &self,
        net: &dyn Fetch,
        mark: Option<&str>,
        _cache: &Path,
    ) -> Result<Option<Batch>, String> {
        let pass = Pass::after(mark, (self.now)());
        let text = match net.text(&page(&pass), None)? {
            Fetched::New { text, .. } => text,
            Fetched::Same => return Err("the MCP Registry answered that nothing changed".into()),
        };
        let Page {
            listings,
            gone,
            next,
        } = read_page(self.id(), &text)?;
        // A page with nothing on it ends a pass, wherever the registry says it goes on.
        let next = next.filter(|_| !listings.is_empty() || !gone.is_empty());
        let next = next.filter(|next| pass.cursor.as_ref() != Some(next));
        let pass = pass.read(next);
        Ok(Some(Batch {
            listings,
            gone,
            mark: pass.mark(),
            whole: false,
            more: !pass.done,
        }))
    }
}

/// The address of the page `pass` reads next. A pass of what changed asks for the deleted too,
/// so they are known to be gone.
fn page(pass: &Pass) -> String {
    let mut url = SERVERS.to_string();
    if let Some(cursor) = &pass.cursor {
        url.push_str("&cursor=");
        url.extend(utf8_percent_encode(cursor, NON_ALPHANUMERIC));
    }
    if let Some(since) = &pass.since {
        url.push_str("&include_deleted=true&updated_since=");
        url.extend(utf8_percent_encode(since, NON_ALPHANUMERIC));
    }
    url
}

/// A page of the registry, read.
#[derive(Debug, Default, PartialEq, Eq)]
struct Page {
    /// Its servers, as listings.
    listings: Vec<Listing>,
    /// The ids of those deleted, or that can no longer be reached.
    gone: Vec<String>,
    /// The cursor of the page after it.
    next: Option<String>,
}

/// A page of the registry, its servers listings of `source`.
fn read_page(source: &str, text: &str) -> Result<Page, String> {
    let page: Value = serde_json::from_str(text)
        .map_err(|why| format!("the MCP Registry's answer is not JSON: {why}"))?;
    let servers = page.get("servers").and_then(Value::as_array);
    let servers = servers.ok_or("the MCP Registry's answer lists no servers")?;
    let (mut listings, mut gone) = (vec![], vec![]);
    for entry in servers {
        let Some(server) = entry.get("server") else {
            continue;
        };
        let name = text_at(server, "name");
        if name.is_empty() {
            continue;
        }
        let official = entry.get("_meta").and_then(|meta| meta.get(OFFICIAL));
        // An older version, should a page carry one, says nothing of the server as it is.
        let latest = official.and_then(|official| official.get("isLatest"));
        if latest.and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let status = official.map(|official| text_at(official, "status"));
        match status.as_deref() == Some("deleted") {
            true => gone.push(format!("{source}/{name}")),
            false => match listing(source, &name, server) {
                Some(listing) => listings.push(listing),
                // One that can no longer be reached is no longer listed.
                None => gone.push(format!("{source}/{name}")),
            },
        }
    }
    let next = page
        .get("metadata")
        .map(|metadata| text_at(metadata, "nextCursor"));
    let next = next.filter(|next| !next.is_empty());
    Ok(Page {
        listings,
        gone,
        next,
    })
}

/// One server as a listing, when it can be reached.
fn listing(source: &str, name: &str, server: &Value) -> Option<Listing> {
    let reached = runs::reached(server)?;
    let title = Some(text_at(server, "title"))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| readable(name.rsplit('/').next().unwrap_or(name)));
    let homepage = Some(text_at(server, "websiteUrl"))
        .filter(|url| url.starts_with("https://"))
        .or_else(|| {
            let repository = server.get("repository")?;
            Some(text_at(repository, "url")).filter(|url| url.starts_with("https://"))
        });
    Some(Listing {
        id: format!("{source}/{name}"),
        what: What::Server,
        source: source.to_string(),
        name: name.to_string(),
        title,
        summary: text_at(server, "description"),
        author: name.split_once('/').map_or("", |(by, _)| by).to_string(),
        category: String::new(),
        homepage: homepage.unwrap_or_default(),
        version: text_at(server, "version"),
        // A server is never the same as another source's: the registry is never merged.
        origin: String::new(),
        recipe: Recipe::Server(reached),
    })
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
