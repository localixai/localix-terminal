//! The sources that are due read into the index, one after another and one request at a time:
//! a source is due an hour after it was last read, at once when its pass was cut short, and
//! always when the person asks. A source's pages are read while it says there are more; one
//! source failing leaves the others to be read.

use std::path::Path;

use super::Index;
use crate::catalog::source::{Fetch, Source};

/// How long a source is left after it was read, in seconds.
const FRESH_FOR: u64 = 60 * 60;

/// The most pages one source is read for in one refresh: a source that never says it is done
/// stops there, and goes on from its mark the next time.
const PAGES_MOST: usize = 1000;

/// How far a refresh is, as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// The source being read: its id, and what the person reads it as.
    pub source: &'static str,
    pub title: &'static str,
    /// How many of its listings were written so far in this refresh.
    pub listings: u64,
}

/// Every one of `sources` that is due (all of them when `forced`), read through `net` into
/// `index` at `now`, `cache` its place to keep what it fetched; `told` hears how far each is.
/// Answers each source read, with how many listings it wrote or why it failed.
pub fn refresh(
    index: &Index,
    sources: &[&dyn Source],
    net: &dyn Fetch,
    cache: &Path,
    now: u64,
    forced: bool,
    told: &dyn Fn(&Progress),
) -> Vec<(&'static str, Result<u64, String>)> {
    (sources.iter())
        .filter(|source| {
            let kept = index.kept(source.id());
            forced
                || kept
                    .is_none_or(|kept| kept.more || now.saturating_sub(kept.read_at) >= FRESH_FOR)
        })
        .map(|source| {
            let read = read(index, *source, net, cache, now, told);
            if let Err(why) = &read {
                log::warn!("catalog: {} was not read: {why}", source.id());
            }
            (source.id(), read)
        })
        .collect()
}

/// `source` read into `index`, page by page, from where it was left.
fn read(
    index: &Index,
    source: &dyn Source,
    net: &dyn Fetch,
    cache: &Path,
    now: u64,
    told: &dyn Fn(&Progress),
) -> Result<u64, String> {
    let mut progress = Progress {
        source: source.id(),
        title: source.title(),
        listings: 0,
    };
    told(&progress);
    let cache = cache.join(source.id());
    let mut mark = index.kept(source.id()).map(|kept| kept.mark);
    for _ in 0..PAGES_MOST {
        let Some(batch) = source.read(net, mark.as_deref(), &cache)? else {
            index.touch(source.id(), now)?;
            break;
        };
        index.put(source.id(), source.trust(), &batch, now)?;
        progress.listings += u64::try_from(batch.listings.len()).unwrap_or(u64::MAX);
        told(&progress);
        if !batch.more {
            break;
        }
        mark = Some(batch.mark);
    }
    Ok(progress.listings)
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
