//! What the person does in the Store's page: each thing posted is one JSON action (`do`), and
//! each is answered by telling the page what is true now. The page is the Store's own, yet what
//! it posts is checked as anything from outside is: an id is looked up, an agent is one of the
//! Store's, a value is one the listing asks for, an address is a web address.

use std::sync::Arc;

use serde_json::{Value, json};

use super::Store;
use crate::catalog::{Listing, Query, What};
use crate::kind::Tab;

/// The most rows one answer carries.
const ROWS_MOST: usize = 200;

impl Store {
    pub(super) fn did(&self, body: &str, tab: &Arc<dyn Tab>) {
        let Ok(action) = serde_json::from_str::<Value>(body) else {
            return;
        };
        let text = |name: &str| action.get(name).and_then(Value::as_str).unwrap_or_default();
        let done = match text("do") {
            "search" => self
                .found(&action)
                .map(|found| tab.paint(&super::page::frame("found", &found))),
            "show" => self.listing(text("id")).map(|listing| {
                tab.paint(&super::page::frame("shown", &self.shown(&listing)));
                // Showing it set the note right: the counts may have moved.
                tab.paint(&super::page::frame("begun", &self.begun(false)));
            }),
            "plan" => self
                .ask_install(text("id"), text("agent"), false, tab)
                .map(|_| ()),
            "remove" => self.listing(text("id")).and_then(|listing| {
                let target = self.target(text("agent"))?;
                self.change(listing, target, None);
                Ok(())
            }),
            "refresh" => {
                self.read_sources(true);
                Ok(())
            }
            "sources" => {
                tab.paint(&super::page::frame("sources", &self.sources()));
                Ok(())
            }
            "source-read" => {
                let id = text("id").to_string();
                self.read_some(true, Some(id));
                Ok(())
            }
            "source-add" => {
                self.ask_add_source(tab);
                Ok(())
            }
            "source-remove" => {
                self.ask_remove_source(text("id"), tab);
                Ok(())
            }
            "sources-restore" => {
                self.restore_sources();
                Ok(())
            }
            "update-all" => {
                self.update();
                Ok(())
            }
            "browse" => {
                let url = text("url");
                if url.starts_with("https://") || url.starts_with("http://") {
                    tab.browse(url);
                }
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(why) = done {
            tab.paint(&super::page::frame(
                "said",
                &json!({"said": why, "busy": false}),
            ));
        }
    }

    /// What a search of the page finds: a page of rows, each with whether the Store installed
    /// it and whether what is listed now is newer.
    fn found(&self, action: &Value) -> Result<Value, String> {
        let number = |name: &str| action.get(name).and_then(Value::as_u64).unwrap_or(0);
        let offset = usize::try_from(number("offset")).unwrap_or(0);
        let limit = usize::try_from(number("limit"))
            .unwrap_or(0)
            .clamp(1, ROWS_MOST);
        let text = action
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let show = action.get("show").and_then(Value::as_str).unwrap_or("all");
        let named = |name: &str| {
            let value = action.get(name).and_then(Value::as_str);
            value.filter(|value| !value.is_empty()).map(str::to_string)
        };
        let trusts = self.trusts();
        let mut facets = Value::Null;
        let (total, rows): (u64, Vec<Value>) = match show {
            "installed" | "updates" => {
                let wanted = text.trim().to_lowercase();
                let noted = self.noted();
                let state = crate::lock(&self.state);
                let rows: Vec<Value> = (noted.iter())
                    .filter(|(_, behind)| *behind || show == "installed")
                    .filter(|(listing, _)| wanted.is_empty() || matches(listing, &wanted))
                    .map(|(listing, _)| Self::row(listing, &trusts, &state.stand(listing)))
                    .collect();
                let total = rows.len() as u64;
                (total, rows.into_iter().skip(offset).take(limit).collect())
            }
            _ => {
                let query = Query {
                    text: text.to_string(),
                    what: What::parse(show),
                    category: named("category"),
                    source: named("source"),
                    limit,
                    offset,
                };
                let found = self.index.search(&query)?;
                // What a list can be narrowed to is said with its first page.
                if offset == 0 && text.trim().is_empty() {
                    let counted = (self.index).facets(query.what, query.source.as_deref());
                    facets = json!(counted);
                }
                let state = crate::lock(&self.state);
                let rows = (found.listings.iter())
                    .map(|listing| Self::row(listing, &trusts, &state.stand(listing)))
                    .collect();
                (found.total, rows)
            }
        };
        Ok(json!({
            "seq": action.get("seq"), "offset": offset, "total": total, "listings": rows,
            "facets": facets,
        }))
    }
}

/// Whether `listing` has `wanted` (in lower case) in what a person reads of it.
fn matches(listing: &Listing, wanted: &str) -> bool {
    [
        &listing.name,
        &listing.title,
        &listing.summary,
        &listing.author,
    ]
    .iter()
    .any(|field| field.to_lowercase().contains(wanted))
}
