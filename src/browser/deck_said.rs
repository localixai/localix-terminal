//! What the window's pages are told, told once: a script the same as the one a page was last
//! told of its kind is not run again, and a deck's refreshes asked for while a command is carried
//! out are drawn once, when it is done.

use std::collections::HashMap;
use std::hash::{Hash, Hasher as _};
use std::sync::atomic::{AtomicBool, Ordering};

/// What each page was last told, by its part and the kind of script, as a hash of the script;
/// and the strip's page loads counted then: a part that loaded its page again has lost all it was
/// told, so the whole record starts over.
#[derive(Debug, Default)]
pub(super) struct Said<K> {
    loads: u32,
    told: HashMap<K, u64>,
}

impl<K: Eq + Hash> Said<K> {
    /// Whether `script` (with `about`, what else it is told for: a page's own load) is news to
    /// `key` now that the pages have loaded `loads` times: recorded as told when it is.
    pub(super) fn news(&mut self, loads: u32, key: K, script: &str, about: impl Hash) -> bool {
        if self.loads != loads {
            self.loads = loads;
            self.told.clear();
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        script.hash(&mut hasher);
        about.hash(&mut hasher);
        let hash = hasher.finish();
        self.told.insert(key, hash) != Some(hash)
    }

    /// `key` told nothing yet: what it is told next is news, whatever it is. For a part whose
    /// view another script has drawn over.
    pub(super) fn forget(&mut self, key: &K) {
        self.told.remove(key);
    }
}

/// A deck's refreshes, held while a command is carried out: none asked for meanwhile is drawn,
/// and the one its end draws stands for them all. The loop's alone, as the deck is.
#[derive(Debug, Default)]
pub(super) struct Held(AtomicBool);

impl Held {
    /// Refreshes held from now until `release`.
    pub(super) fn hold(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether a refresh asked for now is drawn now: not while held.
    pub(super) fn now(&self) -> bool {
        !self.0.load(Ordering::Relaxed)
    }

    /// Refreshes drawn as they are asked for again.
    pub(super) fn release(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[path = "deck_said_tests.rs"]
mod tests;
