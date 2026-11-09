//! The calls the native side made to its pages and waits on. Each has an id and a word, both
//! random, which only the script evaluated in that one page holds: a reply is the call's only
//! when it names both. Where the channel a reply comes in on says which tab sent it, the tab must
//! be the one the call went to as well (on macOS one channel serves every tab, and the word alone
//! binds a reply to the page it was sent to).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use super::secret;

use super::host::Reply;

/// One call waiting for its reply.
struct Waiting {
    sender: SyncSender<Reply>,
    word: String,
    /// The tab whose page the script went to, once the window sent it there.
    tab: Option<u32>,
}

/// A call just made: the id and word its script carries, and where its reply comes.
pub(super) struct Call {
    pub(super) id: u64,
    pub(super) word: String,
    pub(super) receiver: Receiver<Reply>,
}

/// Every call waiting now, by id.
#[derive(Default)]
pub(super) struct Calls {
    waiting: Mutex<HashMap<u64, Waiting>>,
}

impl Calls {
    /// A new call, waiting until it is answered or forgotten.
    pub(super) fn open(&self) -> Call {
        let (sender, receiver) = sync_channel(1);
        let word = secret::word();
        let mut waiting = crate::lock(&self.waiting);
        let mut id = secret::call_id();
        while waiting.contains_key(&id) {
            id = secret::call_id();
        }
        let call = Waiting {
            sender,
            word: word.clone(),
            tab: None,
        };
        waiting.insert(id, call);
        Call { id, word, receiver }
    }

    /// Call `id`'s script went to tab `tab`'s page.
    pub(super) fn aimed(&self, id: u64, tab: u32) {
        if let Some(call) = crate::lock(&self.waiting).get_mut(&id) {
            call.tab = Some(tab);
        }
    }

    /// Run `then` if call `id` still waits and a reply naming `word`, from tab `from` where the
    /// channel says, is its own; under the lock the call takes to stop waiting, so nothing is
    /// done for a call that already gave up.
    pub(super) fn while_waiting(
        &self,
        id: u64,
        word: &str,
        from: Option<u32>,
        then: impl FnOnce(),
    ) {
        let waiting = crate::lock(&self.waiting);
        if waiting.get(&id).is_some_and(|call| owns(call, word, from)) {
            then();
        }
    }

    /// A page's reply to call `id`, carrying `word`, from tab `from` where the channel says:
    /// handed to the call only if it is the call's own. Whether it was.
    pub(super) fn replied(&self, id: u64, word: &str, from: Option<u32>, reply: Reply) -> bool {
        let mut waiting = crate::lock(&self.waiting);
        if !waiting.get(&id).is_some_and(|call| owns(call, word, from)) {
            return false;
        }
        let call = waiting.remove(&id);
        drop(waiting);
        if let Some(call) = call {
            let _ = call.sender.send(reply);
        }
        true
    }

    /// The native side's own answer to call `id`: its script did not start, its tab closed, or a
    /// panel's page gave its value.
    pub(super) fn answer(&self, id: u64, reply: Reply) {
        let call = crate::lock(&self.waiting).remove(&id);
        if let Some(call) = call {
            let _ = call.sender.send(reply);
        }
    }

    /// Call `id` waits no longer.
    pub(super) fn forget(&self, id: u64) {
        crate::lock(&self.waiting).remove(&id);
    }

    /// Every call waiting fails with `error`.
    pub(super) fn fail_all(&self, error: &str) {
        let waiting = std::mem::take(&mut *crate::lock(&self.waiting));
        for call in waiting.into_values() {
            let _ = call.sender.send(Err(error.to_string()));
        }
    }
}

/// Whether a reply carrying `word`, from tab `from` where that is known, is `call`'s.
fn owns(call: &Waiting, word: &str, from: Option<u32>) -> bool {
    let tab = match from {
        Some(from) => call.tab == Some(from),
        None => true,
    };
    tab && !call.word.is_empty() && call.word == word
}

#[cfg(test)]
#[path = "calls_tests.rs"]
mod tests;
