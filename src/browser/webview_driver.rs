//! The page's side of each [`Driver`] call: a probe call, a script, a picture, and the load an
//! action may start, followed until its document is usable.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::view::{Action, ViewError, ViewResult};
use serde_json::{Value, json};

use super::super::host::Cmd;
use super::super::presence::Event;
use super::super::script::script_forms;
use super::super::toolbar;
use super::super::{Clip, Driver, Observation, PageText, Shot};
use super::{CALL_TIMEOUT, LOAD_TIMEOUT, PROBE_READY, WebviewDriver, closed, fragment_only};

const SETTLE_TIMEOUT: Duration = Duration::from_millis(2500);
/// Once its DOM is parsed, how long a document gets to finish loading before it is used anyway.
const PARSED_GRACE: Duration = Duration::from_secs(1);
/// A long page may render off screen before WebKit draws its picture.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(12);

/// A script that did not start says why: it does not parse, or the page has no probe.
fn no_probe(error: ViewError) -> ViewError {
    match error {
        ViewError::Unavailable(e) if e.starts_with("script_not_started") => {
            ViewError::Refused("the script does not parse, or the page has no probe".into())
        }
        other @ (ViewError::Stale(_) | ViewError::Refused(_) | ViewError::Unavailable(_)) => other,
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

/// The agent is acting until this is dropped: the strip says so meanwhile.
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
        let said = self.call("keys", &json!({"keys": keys, "delay": delay_ms}), budget)?;
        let settled = self.call("settle", &json!({}), SETTLE_TIMEOUT);
        self.follow_navigation(before, &settled);
        // The probe presses only so many keys at once, and says how many it did.
        match said.get("pressed").and_then(Value::as_u64) {
            Some(pressed) if usize::try_from(pressed).is_ok_and(|n| n < keys.len()) => {
                Err(ViewError::Refused(format!(
                    "only the first {pressed} of {} keys were pressed: a call presses that many at most",
                    keys.len()
                )))
            }
            _ => Ok(()),
        }
    }
}

impl Driver for WebviewDriver {
    fn navigate(&self, url: &str) -> ViewResult<()> {
        self.my_turn()?;
        let _acting = self.acting();
        let url = super::loadable(url)?;
        self.open_or_closed()?;
        let before = self.loads();
        let here = (crate::lock(&self.shared.tabs).iter())
            .find(|tab| tab.id == self.target())
            .map(|tab| tab.url.clone());
        let same_page = here.is_some_and(|here| fragment_only(&here, &url));
        let (reply, answer) = mpsc::sync_channel(1);
        let tab = self.tab;
        self.send(Cmd::Navigate { url, tab, reply })?;
        // A tab or a window closed meanwhile says so at once: no load is waited for.
        if let Ok(Err(why)) = answer.recv_timeout(CALL_TIMEOUT) {
            return Err(ViewError::Unavailable(why));
        }
        // Another fragment of the same page starts no load: there is nothing to wait to start.
        if !same_page {
            self.load_started_since(before, PROBE_READY);
        }
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

    fn epoch(&self) -> ViewResult<String> {
        let epoch = self.call_ready("epoch", &Value::Null)?;
        epoch
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| ViewError::Unavailable(format!("unreadable epoch: {epoch}")))
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
            // What was typed is never printed: it may be a secret filled in.
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
        // Read with the script path alone: no second settle, and no act for the strip to tell.
        let first = script_forms("innerWidth + ' × ' + innerHeight")
            .into_iter()
            .next();
        let seen = first
            .map(|form| self.run_script(&form))
            .unwrap_or_else(|| Err(ViewError::Refused("the script does not parse".into())))
            .map_err(no_probe)?;
        let seen = seen.get("result").and_then(Value::as_str).unwrap_or("");
        // A size larger than the window is drawn smaller to fit it, and its layout may round by
        // a pixel: the page has the size asked for.
        let near = |frame: &super::super::sizes::Frame| {
            let sides: Vec<f64> = (seen.split('×'))
                .filter_map(|side| side.trim().parse().ok())
                .collect();
            matches!(sides[..], [width, height]
                if (width - frame.width).abs() <= 1.0 && (height - frame.height).abs() <= 1.0)
        };
        Ok(match frame {
            None => format!("The page fills the window: {seen}."),
            Some(frame) if near(&frame) => format!(
                "Page size {}: {}. The person's window is as they left it.",
                frame.name,
                frame.sides()
            ),
            Some(frame) => format!(
                "Page size {} ({}), laid out at {seen}.",
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
                self.reload()?;
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
        let value = value.map_err(no_probe)?;
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
