//! The tool calls an MCP server runs at once: its share of them, which of them the host gave
//! up (`notifications/cancelled`), so that one is not answered when it ends, and the word a
//! call sends while it runs to a host that follows it (a `progressToken`), so the host that
//! counts its patience from the last word does not give up a call that is only long.

use std::collections::HashMap;
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::wire::Out;

/// How often a call that is followed says it still runs.
const BEAT: Duration = Duration::from_secs(5);

/// The calls running, each by its request's id (as JSON, so `1` and `"1"` are two) with
/// whether the host cancelled it.
pub(super) struct Running {
    most: usize,
    calls: Mutex<HashMap<String, bool>>,
}

impl Running {
    pub(super) fn new(most: usize) -> Self {
        Self {
            most,
            calls: Mutex::new(HashMap::new()),
        }
    }

    /// A call starts under `id`, when there is a place for one more.
    pub(super) fn start(&self, id: &Value) -> bool {
        let mut calls = crate::lock(&self.calls);
        if calls.len() >= self.most {
            return false;
        }
        calls.insert(id.to_string(), false);
        true
    }

    /// The host gave up the call under `id`; one that is not running is none of ours.
    pub(super) fn cancel(&self, id: &Value) {
        if let Some(cancelled) = crate::lock(&self.calls).get_mut(&id.to_string()) {
            *cancelled = true;
        }
    }

    /// `say` run while the call under `id` runs and was not given up: what it writes is in
    /// front of the call's answer, never after it.
    fn while_running(&self, id: &Value, say: impl FnOnce()) {
        if crate::lock(&self.calls).get(&id.to_string()) == Some(&false) {
            say();
        }
    }

    /// The call under `id` ended and its place is free; whether it is still to be answered.
    pub(super) fn finish(&self, id: &Value) -> bool {
        crate::lock(&self.calls).remove(&id.to_string()) != Some(true)
    }
}

/// The call under `id` followed, when its `meta` asks (a `progressToken`, a string or a
/// number): until what this returns is dropped, a progress notification each beat, counted up.
/// None when the host does not follow the call, or there is no thread to tell it with.
pub(super) fn followed(
    out: &Out,
    running: &Arc<Running>,
    id: &Value,
    meta: &Value,
) -> Option<SyncSender<()>> {
    let token =
        (meta.get("progressToken")).filter(|token| token.is_string() || token.is_number())?;
    let (ended, wait) = sync_channel::<()>(1);
    let (out, running, id, token) = (out.clone(), Arc::clone(running), id.clone(), token.clone());
    std::thread::Builder::new()
        .name("mcp-progress".into())
        .spawn(move || {
            let mut beats = 0_u64;
            while wait.recv_timeout(BEAT) == Err(RecvTimeoutError::Timeout) {
                beats += 1;
                running.while_running(&id, || out.send(&progress(&token, beats)));
            }
        })
        .ok()?;
    Some(ended)
}

/// The word a followed call sends: its token, and how many beats it has run.
fn progress(token: &Value, beats: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": {"progressToken": token, "progress": beats, "message": "Still working."},
    })
}

#[cfg(test)]
#[path = "running_tests.rs"]
mod tests;
