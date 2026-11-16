//! Waiting in a page for text to show: polled in the page itself, said back with how long it took.

use std::time::Instant;

use serde_json::Value;

use crate::browser::Driver;

/// Waits until the page shows `wanted`, at most `most` ms, polled in the page on a timer
/// (animation frames stop in a window in the background); says whether it came and when. What
/// it shows is what the outline names: its text, and a control's name or value (a button known
/// only by its `aria-label`, a field's placeholder or what was typed in it).
pub(super) fn wait_for(driver: &impl Driver, wanted: &str, most: u64) -> (String, bool) {
    let literal = Value::String(wanted.to_string());
    let source = format!(
        "const wanted = {literal}; const until = performance.now() + {most}; \
         const NAMES = ['aria-label', 'title', 'placeholder', 'alt']; \
         const named = () => [...document.querySelectorAll('[aria-label],[title],[placeholder],[alt]')] \
           .some(el => NAMES.some(name => (el.getAttribute(name) ?? '').includes(wanted))) \
           || [...document.querySelectorAll('input,textarea,select')] \
           .some(el => String(el.value ?? '').includes(wanted)); \
         const shows = () => (document.body?.innerText ?? '').includes(wanted) || named(); \
         while (!shows() && performance.now() < until) await sleep(100); shows()"
    );
    let started = Instant::now();
    let seconds = |ms: u128| format!("{}.{}s", ms / 1000, ms % 1000 / 100);
    match driver.evaluate(&source).as_deref() {
        Ok("true") => (
            format!(
                "\"{wanted}\" appeared after {}",
                seconds(started.elapsed().as_millis())
            ),
            true,
        ),
        Ok(_) => (
            format!(
                "\"{wanted}\" did not appear within {}: the page may still be working; wait again, or read what it shows",
                seconds(u128::from(most))
            ),
            false,
        ),
        Err(e) => (format!("Waiting for \"{wanted}\" failed: {e}"), false),
    }
}
