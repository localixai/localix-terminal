//! Probe scripts awaited across document changes, and navigation followed after an action.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::super::host::{Loads, WINDOW_CLOSED};
use super::super::script::Form;
use super::{LOAD_TIMEOUT, WebviewDriver, closed};
use crate::browser::Driver;
use crate::view::{ViewError, ViewResult};
use serde_json::{Value, json};

/// A script may await the page (a fetch, a timer); it gets this long.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(20);
/// After a script's page started loading another document, how long its answer may still come:
/// a script that clicked Next and then awaited would otherwise hold the run for SCRIPT_TIMEOUT.
const SCRIPT_LEFT: Duration = Duration::from_millis(300);
/// What a script returns when its document went away before it answered.
const SCRIPT_LEFT_PAGE: &str = "(no value: the page loaded a new document while the script ran)";
/// After an action, how long a navigation it caused has to show up.
const NAVIGATION_GRACE: Duration = Duration::from_millis(25);
/// After an action that requested a navigation the page did not report (a subframe's), how long
/// its load has to start.
const NAVIGATION_START: Duration = Duration::from_millis(300);
/// A settle that got no answer: its document most likely went away; a new load should follow.
const SETTLE_LOST: Duration = Duration::from_secs(5);
impl WebviewDriver {
    pub(super) fn run_script(&self, form: &Form) -> ViewResult<Value> {
        self.open_or_closed()?;
        let before = self.loads();
        // What the page tells is the tab's the call went to, whichever tab is active by its reply.
        let tab = self.target();
        let (id, receiver) = self.post(|id| form.at(id), "script_not_started")?;
        let deadline = Instant::now() + SCRIPT_TIMEOUT;
        let mut left: Option<Instant> = None;
        let reply = loop {
            let now = Instant::now();
            let until = left.map_or(deadline, |at| (at + SCRIPT_LEFT).min(deadline));
            if now >= until {
                break Err(mpsc::RecvTimeoutError::Timeout);
            }
            match receiver.recv_timeout((until - now).min(Duration::from_millis(50))) {
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                other => break other,
            }
            if left.is_none() && self.loads().started != before.started {
                left = Some(Instant::now());
            }
        };
        crate::lock(&self.shared.pending).remove(&id);
        self.shared.notices.file(id, tab);
        match reply {
            Ok(Ok(value)) => Ok(value),
            Err(_) if left.is_some() => Ok(json!({"result": SCRIPT_LEFT_PAGE})),
            Ok(Err(error)) if error.starts_with("script_not_started") => {
                Err(ViewError::Unavailable(error))
            }
            Ok(Err(error)) if error == WINDOW_CLOSED => Err(closed()),
            Ok(Err(error)) => Err(ViewError::Refused(error)),
            Err(_) => Err(ViewError::Unavailable(
                "the script did not finish in time".into(),
            )),
        }
    }

    /// After an action: if it replaced the document, wait for the new one to load. The page knows
    /// first (`navigating`; a settle that died with its document says the same); a navigation the
    /// native side saw requested but the page did not (a subframe) gets a short look only.
    pub(super) fn follow_navigation(&self, before: Loads, settled: &ViewResult<Value>) {
        let requested = self.loads().requested != before.requested;
        let flag = |key| {
            settled
                .as_ref()
                .is_ok_and(|value| value.get(key).and_then(Value::as_bool) == Some(true))
        };
        let grace = match settled {
            _ if flag("navigating") => LOAD_TIMEOUT,
            Err(_) => SETTLE_LOST,
            // Frames request navigations all the time (ad syncs); with the Navigation API the page
            // has reported its own, so theirs are not waited for.
            _ if requested && !flag("watched") => NAVIGATION_START,
            _ => NAVIGATION_GRACE,
        };

        if self.load_started_since(before, grace) {
            let _ = self.wait_loaded(LOAD_TIMEOUT);
        }
    }
}
