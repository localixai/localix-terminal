//! The Store as an agent uses it: it searches and reads through the tool's `store` field, and
//! what it asks to install is put to the person in the Store's page, never done on its word.
//! What the person answered, and how an install ended, it reads in the tab's text.

use std::fmt::Write as _;
use std::sync::Arc;

use serde_json::Value;

use super::{Store, many, what_word};
use crate::catalog::{Listing, Query, What};
use crate::kind::{Said, Tab};

/// How many listings a search answers when it does not say, and at most.
const FOUND: usize = 20;
const FOUND_MOST: usize = 50;

impl Store {
    pub(super) fn asked_by_agent(&self, args: &Value, tab: &Arc<dyn Tab>) -> Result<Said, String> {
        let asked = args.get("store").and_then(Value::as_object);
        let asked = asked.ok_or("`store` is an object: {\"search\": …}, {\"show\": …} or {\"install\": …, \"agent\": …}")?;
        let text = |name: &str| asked.get(name).and_then(Value::as_str);
        if let Some(id) = text("install") {
            let agent = text("agent").ok_or("`agent` is needed: which agent to install it in")?;
            let name = self.target(agent)?.name();
            let steps = self.ask_install(id, agent, true, tab)?;
            let report = format!(
                "Asked the person to install {id} in {name}. It would: {}. They are asked in \
                 the window's dialog; read the Store's tab again for their answer.",
                steps.join("; "),
            );
            return Ok(Said {
                report,
                state: None,
            });
        }
        if let Some(id) = text("show") {
            let listing = self.listing(id)?;
            return Ok(Said {
                report: format!("{} ({})", listing.title, what_word(listing.what)),
                state: Some(self.read(&listing)),
            });
        }
        if let Some(words) = text("search") {
            let what = match text("what") {
                Some(name) => Some(What::parse(name).ok_or("`what` is plugin, skill or server")?),
                None => None,
            };
            let limit = asked.get("limit").and_then(Value::as_u64);
            let limit = limit.map_or(FOUND, |n| usize::try_from(n).unwrap_or(FOUND_MOST));
            let query = Query {
                text: words.to_string(),
                what,
                limit: limit.clamp(1, FOUND_MOST),
                ..Query::default()
            };
            let found = self.index.search(&query)?;
            let lines: Vec<String> = found.listings.iter().map(line).collect();
            return Ok(Said {
                report: format!("{} found, {} shown", many(found.total), lines.len()),
                state: Some(lines.join("\n")),
            });
        }
        Err("`store` takes `search`, `show` or `install`".into())
    }

    /// One listing as an agent reads it, with where each agent stands with it.
    fn read(&self, listing: &Listing) -> String {
        let shown = self.shown(listing);
        let mut out = format!(
            "{}\n{} · {} · by {} · version {}\n{}\n{}\n",
            listing.id,
            listing.title,
            what_word(listing.what),
            or_none(&listing.author),
            or_none(&listing.version),
            listing.summary,
            listing.homepage,
        );
        if listing.runs_code() {
            out.push_str("Installing it puts code on the person's machine that an agent runs.\n");
        }
        for need in listing.needs() {
            let must = if need.required { "needed" } else { "optional" };
            let _ = writeln!(out, "needs {} ({must}): {}", need.name, need.about);
        }
        for agent in shown["agents"].as_array().into_iter().flatten() {
            let stands = match (
                agent["takes"].as_bool(),
                agent["present"].as_bool(),
                &agent["has"],
            ) {
                (Some(false), _, _) => "does not take it",
                (_, Some(false), _) => "not on this machine",
                (_, _, Value::Null) => "not installed",
                _ => "installed",
            };
            let _ = writeln!(
                out,
                "{}: {stands}",
                agent["id"].as_str().unwrap_or_default()
            );
        }
        out
    }

    /// The Store's tab as text: what there is, what the Store installed, what waits for the
    /// person, and what happened lately.
    pub(super) fn as_text(&self) -> String {
        let counts = self.counts();
        let count = |name: &str| many(counts[name].as_u64().unwrap_or(0));
        let mut out = format!(
            "Store: {} plugins, {} skills, {} MCP servers.\n",
            count("plugin"),
            count("skill"),
            count("server"),
        );
        for (listing, behind) in self.noted() {
            let newer = if behind {
                " (a newer one is listed)"
            } else {
                ""
            };
            let _ = writeln!(out, "installed: {}{newer}", listing.id);
        }
        let state = crate::lock(&self.state);
        if let Some(asked) = &state.asked {
            let _ = writeln!(
                out,
                "waits for the person's answer: install {} in {}",
                asked["id"].as_str().unwrap_or_default(),
                asked["agent"].as_str().unwrap_or_default(),
            );
        }
        for what in &state.lately {
            out.push_str(what);
            out.push('\n');
        }
        out.push_str("Search with {\"store\": {\"search\": \"words\"}}.");
        out
    }
}

fn or_none(text: &str) -> &str {
    if text.is_empty() { "none" } else { text }
}

/// A listing as one line of a search.
fn line(listing: &Listing) -> String {
    let by = match listing.author.is_empty() {
        true => String::new(),
        false => format!(" by {}", listing.author),
    };
    let summary: String = listing.summary.chars().take(160).collect();
    format!(
        "{} — {} ({}{by}): {summary}",
        listing.id,
        listing.title,
        what_word(listing.what)
    )
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
