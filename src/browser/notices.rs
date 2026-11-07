//! What pages tell besides their content (probe/told.js): a native dialog and how it was
//! answered, words announced in a live region after an action, a field that did not keep what was
//! typed. They come with the probe's replies; the call that waited for a reply files them under
//! its tab, numbered across the tabs, so a call reads what came since it began whichever document
//! the tab went on to. Here too, how each tab answers a confirm or a prompt while a call armed it
//! (Cancel otherwise).

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a tab keeps of what it was told: the newest.
const KEPT: usize = 40;

/// One thing a page told.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Notice {
    /// `alert`, `confirm`, `prompt`, `said` (a live region's words) or `field`.
    pub kind: String,
    pub text: String,
    /// How a confirm or a prompt was answered: `OK`, `Cancel`, `OK, "text"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

impl Notice {
    /// A confirm or a prompt the page was answered Cancel to: what it asked did not go ahead.
    pub fn declined(&self) -> bool {
        matches!(self.kind.as_str(), "confirm" | "prompt")
            && self.answer.as_deref() == Some("Cancel")
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = &self.text;
        let answer = self.answer.as_deref().unwrap_or("Cancel");
        match self.kind.as_str() {
            "alert" => write!(f, "an alert: \"{text}\""),
            "confirm" | "prompt" => write!(f, "a {}: \"{text}\", answered {answer}", self.kind),
            "said" => write!(f, "announced: \"{text}\""),
            _ => f.write_str(text),
        }
    }
}

/// How a tab answers a confirm or a prompt while armed: OK, with `text` for a prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Armed {
    pub text: Option<String>,
}

#[derive(Default)]
pub(super) struct Notices {
    /// What came with a reply, until the call that waits for it files it.
    by_call: Mutex<HashMap<u64, Vec<Notice>>>,
    kept: Mutex<HashMap<u32, VecDeque<(u64, Notice)>>>,
    seq: AtomicU64,
    armed: Mutex<HashMap<u32, Armed>>,
}

impl Notices {
    /// The `told` of call `id`'s reply, held for that call.
    pub(super) fn arrived(&self, id: u64, told: &Value) {
        let told: Vec<Notice> = (told.as_array().into_iter().flatten())
            .filter_map(|notice| serde_json::from_value(notice.clone()).ok())
            .collect();
        if !told.is_empty() {
            crate::lock(&self.by_call)
                .entry(id)
                .or_default()
                .extend(told);
        }
    }

    /// What came with call `id`'s reply, kept for `tab`.
    pub(super) fn file(&self, id: u64, tab: u32) {
        let Some(told) = crate::lock(&self.by_call).remove(&id) else {
            return;
        };
        let mut kept = crate::lock(&self.kept);
        let list = kept.entry(tab).or_default();
        for notice in told {
            let n = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
            list.push_back((n, notice));
            if list.len() > KEPT {
                list.pop_front();
            }
        }
    }

    /// Where the tabs' notices are now: what comes later is `since` this.
    pub(super) fn mark(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    /// What `tab` was told after `mark`.
    pub(super) fn since(&self, tab: u32, mark: u64) -> Vec<Notice> {
        (crate::lock(&self.kept).get(&tab).into_iter().flatten())
            .filter(|(n, _)| *n > mark)
            .map(|(_, notice)| notice.clone())
            .collect()
    }

    pub(super) fn arm(&self, tab: u32, armed: Option<Armed>) {
        let mut all = crate::lock(&self.armed);
        match armed {
            Some(armed) => all.insert(tab, armed),
            None => all.remove(&tab),
        };
    }

    pub(super) fn armed(&self, tab: u32) -> Option<Armed> {
        crate::lock(&self.armed).get(&tab).cloned()
    }

    /// A closed tab's notices and answer, forgotten.
    pub(super) fn forget(&self, tab: u32) {
        crate::lock(&self.kept).remove(&tab);
        crate::lock(&self.armed).remove(&tab);
    }
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod tests;
