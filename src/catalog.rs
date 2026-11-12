//! The Store: plugins, skills and MCP servers that others published, found by a search and put
//! into the coding agents the person has. It knows no source and no agent by itself: a source
//! (source.rs) says what there is to list, the index (index.rs) keeps and searches it, and a
//! target (target.rs) puts a listing into one agent, each behind its port. Nothing here knows
//! the window; the Store's tab is `kind.rs`, a kind like any other (`crate::kind`).

pub mod archive;
pub mod category;
pub mod index;
mod installed;
pub mod kind;
pub mod model;
mod page;
pub mod source;
pub mod sources;
pub mod target;

pub use index::{Found, Held, Index, Progress, Query, refresh};
pub use installed::Installed;
pub use model::{Env, Listing, Need, Plan, PluginFormat, Recipe, Runs, Server, What, With};
/// `text` with its first letter made a capital.
pub(crate) fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

pub use source::{Batch, Fetch, Fetched, Marketplace, Net, SkillsRepo, Source};
