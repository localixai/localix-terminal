//! The page's side of each [`Driver`] call: a probe call, a script, a picture, and the load an
//! action may start, followed until its document is usable.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::view::{Action, ViewError, ViewResult};
use serde_json::{Value, json};

use super::super::host::{Cmd, Loads, WINDOW_CLOSED};
use super::super::presence::Event;
use super::super::script::script_forms;
use super::super::toolbar;
use super::super::{Clip, Driver, Observation, PageText, Shot};
use super::{CALL_TIMEOUT, LOAD_TIMEOUT, PROBE_READY, WebviewDriver, closed};

/// A script may await the page (a fetch, a timer); it gets this long.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(20);
/// After a script's page started loading another document, how long its answer may still come:
/// a script that clicked Next and then awaited would otherwise hold the run for SCRIPT_TIMEOUT.
const SCRIPT_LEFT: Duration = Duration::from_millis(300);
/// What a script returns when its document went away before it answered.
const SCRIPT_LEFT_PAGE: &str = "(no value: the page loaded a new document while the script ran)";
const SETTLE_TIMEOUT: Duration = Duration::from_millis(2500);
/// Once its DOM is parsed, how long a document gets to finish loading before it is used anyway.
const PARSED_GRACE: Duration = Duration::from_secs(1);
/// After an action, how long a navigation it caused has to show up.
const NAVIGATION_GRACE: Duration = Duration::from_millis(25);
/// After an action that requested a navigation the page did not report (a subframe's), how long
/// its load has to start.
const NAVIGATION_START: Duration = Duration::from_millis(300);
/// A settle that got no answer: its document most likely went away; a new load should follow.
const SETTLE_LOST: Duration = Duration::from_secs(5);
/// A long page may render off screen before WebKit draws its picture.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(12);

impl WebviewDriver {
    fn run_script(&self, form: &str) -> ViewResult<Value> {
        self.open_or_closed()?;
        let before = self.loads();
        let script = |id: u64| form.replace("__ID__", &id.to_string());
        // What the page tells is the tab's the call went to, whichever tab is active by its reply.
        let tab = self.target();
        let (id, receiver) = self.post(script, "script_not_started")?;
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
    fn follow_navigation(&self, before: Loads, settled: &ViewResult<Value>) {
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

fn action_args(epoch: &str, action: &Action) -> Value {
    match action {
        Action::Click { reference } => json!({"op": "click", "ref": reference, "epoch": epoch}),
        Action::Type { reference, text } => {
            json!({"op": "type", "ref": reference, "value": text, "epoch": epoch})
        }
        Action::Select { reference, value } => {
            json!({"op": "select", "ref": reference, "value": value, "epoch": epoch})
        }
        Action::PressEnter { reference } => {
            json!({"op": "press_enter", "ref": reference, "epoch": epoch})
        }
        Action::ScrollDown => json!({"op": "scroll_down", "epoch": epoch}),
        Action::ScrollUp => json!({"op": "scroll_up", "epoch": epoch}),
        // `act` takes these the ways a call's `keys` and `url` go, never to the probe.
        Action::Press { .. } | Action::Go { .. } => json!({}),
    }
}

/// The agent is acting: the mini spins until this is dropped, and then takes a new picture.
struct Acting(tao::event_loop::EventLoopProxy<Cmd>);

impl Drop for Acting {
    fn drop(&mut self) {
        let _ = self.0.send_event(Cmd::Presence(Event::Settled));
    }
}

/// How long an action waits for the person to finish commenting before it gives up.
const TURN_WAIT: Duration = Duration::from_secs(60);

impl WebviewDriver {
    /// The agent's turn to move a page (click, type, keys, a load): after the person, while they
    /// comment (the strip says the agent waits), and an error when they are still at it after
    /// TURN_WAIT. A script is not held: the agent reads the page with one as often as it acts.
    fn my_turn(&self) -> ViewResult<()> {
        let told = || {
            let _ = self.proxy.send_event(Cmd::Strip);
        };
        (self.shared.turn.wait(TURN_WAIT, told))
            .map_err(|waited| ViewError::Unavailable(super::super::turn::refused(waited)))
    }

    fn acting(&self) -> Acting {
        let _ = self.proxy.send_event(Cmd::Presence(Event::Acting));
        Acting(self.proxy.clone())
    }

    /// Keys pressed in the page, and the load they start followed.
    fn press_page(&self, keys: &[String], delay_ms: u64) -> ViewResult<()> {
        self.my_turn()?;
        let _acting = self.acting();
        self.before_acting();
        let before = self.loads();
        let budget = Duration::from_millis(delay_ms * keys.len() as u64) + CALL_TIMEOUT;
        self.call("keys", &json!({"keys": keys, "delay": delay_ms}), budget)?;
        let settled = self.call("settle", &json!({}), SETTLE_TIMEOUT);
        self.follow_navigation(before, &settled);
        Ok(())
    }
}

impl Driver for WebviewDriver {
    fn navigate(&self, url: &str) -> ViewResult<()> {
        self.my_turn()?;
        let _acting = self.acting();
        let url = super::loadable(url)?;
        self.open_or_closed()?;
        let before = self.loads();
        self.send(Cmd::Navigate(url, self.tab))?;
        self.load_started_since(before, PROBE_READY);
        self.wait_loaded(LOAD_TIMEOUT)
    }

    /// Until the document is loaded, or parsed (DOMContentLoaded, as its probe says) for
    /// PARSED_GRACE. The `load` event also waits for every image, video and ad frame, which on a
    /// busy site comes many seconds later or never; the page stays `busy` in its snapshot until then.
    fn wait_loaded(&self, timeout: Duration) -> ViewResult<()> {
        let deadline = Instant::now() + timeout;
        let mut parsed: Option<Instant> = None;
        loop {
            self.open_or_closed()?;
            let loads = self.loads();
            if loads.started > 0 && loads.finished >= loads.started {
                return Ok(());
            }
            if parsed.is_some_and(|at| at.elapsed() >= PARSED_GRACE) {
                return Ok(());
            }
            // Load events come from the committed document only: a probe answering now is the
            // new document's.
            if parsed.is_none()
                && loads.started > 0
                && let Ok(Value::String(state)) =
                    self.call("ready", &Value::Null, Duration::from_millis(500))
                && state != "loading"
            {
                parsed = Some(Instant::now());
            }
            if Instant::now() >= deadline {
                return Err(ViewError::Unavailable("the page is still loading".into()));
            }
            let guard = crate::lock(&self.shared.loads);
            // Only the wait matters, not the counts, so a poisoned lock is as good as any.
            drop(
                self.shared
                    .loaded
                    .wait_timeout(guard, Duration::from_millis(50)),
            );
        }
    }

    fn observe(&self) -> ViewResult<Observation> {
        let value = self.call_ready("snapshot", &Value::Null)?;
        serde_json::from_value(value)
            .map_err(|error| ViewError::Unavailable(format!("unreadable snapshot: {error}")))
    }

    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()> {
        match action {
            Action::Press { key } => return self.press(std::slice::from_ref(key), 0),
            Action::Go { url } => return self.navigate(url),
            Action::Click { .. }
            | Action::Type { .. }
            | Action::Select { .. }
            | Action::PressEnter { .. }
            | Action::ScrollDown
            | Action::ScrollUp => {}
        }
        self.my_turn()?;
        let _acting = self.acting();
        self.before_acting();
        let started = Instant::now();
        let before = self.loads();
        let args = action_args(epoch, action);
        self.call("act", &args, CALL_TIMEOUT)?;
        let acted = started.elapsed();
        let settled = self.call("settle", &args, SETTLE_TIMEOUT);
        let settle_time = started.elapsed().saturating_sub(acted);
        self.follow_navigation(before, &settled);
        super::super::debug(|| {
            // What was typed is never printed: it may be a secret put in for a goal.
            let shown = if let Action::Type { reference, text } = action {
                format!("Type {reference} ({} characters)", text.chars().count())
            } else {
                format!("{action:?}")
            };
            format!(
                "{shown}\nact {acted:?} settle {settle_time:?} ({settled:?}) total {:?}",
                started.elapsed()
            )
        });
        Ok(())
    }

    fn frame(&self, size: &str) -> ViewResult<String> {
        let frame = super::super::sizes::parse(size).map_err(ViewError::Refused)?;
        self.open_or_closed()?;
        self.send(Cmd::Frame(Some(self.target()), frame.clone()))?;
        // The page reflows on its own resize event; give it the usual settle.
        let _ = self.call("settle", &json!({}), SETTLE_TIMEOUT);
        let seen = self.evaluate("innerWidth + ' × ' + innerHeight")?;
        Ok(match frame {
            None => format!("The page fills the window: {seen}."),
            Some(frame) if seen == frame.sides() => format!("Page size {}: {seen}.", frame.name),
            Some(frame) => format!(
                "Page size {} ({}), shown at {seen}: the window is smaller.",
                frame.name,
                frame.sides()
            ),
        })
    }

    /// The browser's own shortcuts (a reload) are its own, as a person's are; the keys between
    /// them reach the page.
    fn press(&self, keys: &[String], delay_ms: u64) -> ViewResult<()> {
        // A reload among the keys moves the page too.
        self.my_turn()?;
        for run in keys.split_inclusive(|key| toolbar::shortcut(key).is_some()) {
            let (page, reload) = match run.split_last() {
                Some((last, before)) if toolbar::shortcut(last).is_some() => (before, true),
                _ => (run, false),
            };
            if !page.is_empty() {
                self.press_page(page, delay_ms)?;
            }
            if reload {
                self.evaluate("location.reload()")?;
            }
        }
        Ok(())
    }

    fn evaluate(&self, source: &str) -> ViewResult<String> {
        let _acting = self.acting();
        self.before_acting();
        let before = self.loads();
        let mut value = Err(ViewError::Refused("the script does not parse".into()));
        for form in script_forms(source) {
            value = self.run_script(&form);
            if !matches!(&value, Err(ViewError::Unavailable(e)) if e.starts_with("script_not_started"))
            {
                break;
            }
        }
        let value = value.map_err(|error| match error {
            ViewError::Unavailable(e) if e.starts_with("script_not_started") => {
                ViewError::Refused("the script does not parse, or the page has no probe".into())
            }
            other @ (ViewError::Stale(_) | ViewError::Refused(_) | ViewError::Unavailable(_)) => {
                other
            }
        })?;
        let settled = self.call("settle", &json!({}), SETTLE_TIMEOUT);
        self.follow_navigation(before, &settled);
        Ok(value
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or("undefined")
            .to_string())
    }

    fn read(&self, offset: usize, limit: usize) -> ViewResult<PageText> {
        let value = self.call_ready("read", &json!({"offset": offset, "limit": limit}))?;
        serde_json::from_value(value)
            .map_err(|error| ViewError::Unavailable(format!("unreadable text: {error}")))
    }

    fn screenshot(&self, selector: Option<&str>) -> ViewResult<Shot> {
        self.open_or_closed()?;
        let clip = match selector {
            Some(selector) => {
                let at = self.call_ready("locate", &json!({"selector": selector}))?;
                let side = |key: &str| at.get(key).and_then(Value::as_f64).unwrap_or(0.0);
                Some(Clip {
                    x: side("x"),
                    y: side("y"),
                    width: side("width"),
                    height: side("height"),
                })
            }
            None => None,
        };
        let (reply, shot) = mpsc::sync_channel(1);
        self.send(Cmd::Capture {
            clip,
            reply,
            tab: self.tab,
        })?;
        match shot.recv_timeout(CAPTURE_TIMEOUT) {
            Ok(Ok(shot)) => Ok(shot),
            Ok(Err(why)) => Err(ViewError::Refused(format!("no screenshot: {why}"))),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ViewError::Unavailable(
                "the screenshot did not come in time".into(),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(closed()),
        }
    }
}
