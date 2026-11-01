//! A page's tab: opening an address, and acting in it (keys, a script, waiting for text, reading
//! it whole, a picture), then reading the page back as an outline.

use std::time::Instant;

use serde_json::Value;

use crate::browser::{Driver, Page};
use crate::view::View;

use super::args::{WEB_FIELDS, number, strings, text};
use super::changes::Seen;
use super::{Call, Desk, Output};

const DEFAULT_KEY_DELAY_MS: u64 = 60;
const MAX_KEY_DELAY_MS: u64 = 2000;
/// How long `wait_for` waits by default, and at most: a script in the page ends within 20 s.
const DEFAULT_WAIT_MS: u64 = 10_000;
const MAX_WAIT_MS: u64 = 15_000;
/// Where a tab opens when no address is given.
const SEARCH_PAGE: &str = "https://duckduckgo.com/";
/// Characters of the page outline read after each call, and of text one `read` returns.
const OUTLINE_LIMIT: usize = 6000;
const READ_LIMIT: usize = 8000;

impl Desk {
    pub(super) fn page(&self) -> Result<&Page, String> {
        self.page.as_ref().ok_or_else(|| {
            "there is no window here (no display, or the daemon runs with --no-window): \
             terminals work, pages do not"
                .to_string()
        })
    }

    /// Where `url` opens: a page on disk at the address it is served at, else the web address.
    fn address(&self, call: &Call, url: &str) -> Result<String, String> {
        let root = project(call.cwd);
        match self.served.address(&root, call.cwd, url)? {
            Some(served) => Ok(served),
            None => web_address(url),
        }
    }

    /// Open a tab on `args["url"]` (the search page without one); its id and first snapshot.
    pub(super) fn open_web(&self, call: &Call, args: &Value) -> Result<(u32, Output), String> {
        let page = self.page()?;
        let url = self.address(call, text(args, "url").unwrap_or(SEARCH_PAGE))?;
        let id = {
            // One tab is made at a time, and the window's loop makes it: the lock is held for
            // that, not for the page's load, so tabs opened together load together.
            let _change = crate::lock(&self.tabs_change);
            page.start(&url).map_err(|e| e.to_string())?
        };
        let tab = page.pinned(id);
        let _ = tab.driver().map(|driver| driver.wait_new());
        let read = self.in_tab(id, &serde_json::json!({}), None)?;
        let report = format!("Opened {url}.\n{}", read.report).trim().to_string();
        Ok((id, self.read_whole(id, Output { report, ..read })))
    }

    /// `args` carried out in tab `id`, while no other call works in it.
    pub(super) fn in_tab(
        &self,
        id: u32,
        args: &Value,
        url: Option<&str>,
    ) -> Result<Output, String> {
        let lock = self.tab_lock(id);
        let _tab = crate::lock(&lock);
        let page = self.page()?;
        if !page.is_open() {
            return Err(format!(
                "there is no browser tab w{id}: no page is open; `open` one first"
            ));
        }
        run(&page.pinned(id), args, url)
    }

    /// Acting in tab `id`: the page's changes since it was last read whole, when few; else all.
    pub(super) fn interact_web(
        &self,
        call: &Call,
        id: u32,
        args: &Value,
    ) -> Result<Output, String> {
        let call_args = super::args::picked(args, &WEB_FIELDS);
        let url = match text(args, "url") {
            Some(url) => Some(self.address(call, url)?),
            None => None,
        };
        let mut done = self.in_tab(id, &call_args, url.as_deref())?;
        let Some(page) = done.state.take() else {
            return Ok(done);
        };
        let mut seen = crate::lock(&self.seen);
        match seen.get_mut(&id).and_then(|before| before.after(&page)) {
            Some(changed) if done.report.is_empty() => done.report = changed,
            Some(changed) => done.report = format!("{}\n\n{changed}", done.report),
            None => {
                seen.insert(id, Seen::read(&page));
                done.state = Some(page);
            }
        }
        Ok(done)
    }

    /// Tab `id` read whole: what later changes are told against.
    pub(super) fn read_whole(&self, id: u32, done: Output) -> Output {
        if let Some(page) = &done.state {
            crate::lock(&self.seen).insert(id, Seen::read(page));
        }
        done
    }

    pub(super) fn shoot_web(&self, id: u32, selector: Option<&str>) -> Result<Output, String> {
        let mut args = serde_json::json!({"screenshot": true});
        if let Some(selector) = selector {
            args["selector"] = Value::String(selector.to_string());
        }
        self.in_tab(id, &args, None)
    }

    pub(super) fn close_web(&self, id: u32) -> Result<String, String> {
        let lock = self.tab_lock(id);
        let _tab = crate::lock(&lock);
        let _change = crate::lock(&self.tabs_change);
        let driver = self.page()?.driver().map_err(|e| e.to_string())?;
        crate::lock(&self.seen).remove(&id);
        crate::lock(&self.paid).remove(&id);
        match driver.close_tab(id) {
            Ok(true) => Ok(format!("Closed w{id}.")),
            Ok(false) => Ok(format!(
                "Closed w{id}, the last page: the window closed; the next page opens it again."
            )),
            Err(e) => Err(format!("w{id} was not closed: {e}")),
        }
    }
}

/// The project at `dir`: the nearest directory with a `.git` at or above it, else `dir`.
pub fn project(dir: &std::path::Path) -> std::path::PathBuf {
    let found = dir.ancestors().find(|at| at.join(".git").exists());
    found.unwrap_or(dir).to_path_buf()
}

/// `url` as the window loads it: http or https only. Without a scheme, https, but http for this
/// machine: a dev server serves plain http.
pub(super) fn web_address(url: &str) -> Result<String, String> {
    let Some((scheme, _)) = url.split_once("://") else {
        let host = url.split(['/', '?', '#']).next().unwrap_or("");
        let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
        let local = ["localhost", "0.0.0.0"].contains(&name)
            || name.starts_with("127.")
            || host.starts_with("[::1]");
        let url = format!("{}://{url}", if local { "http" } else { "https" });
        return web_address(&url);
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(format!(
            "only http and https addresses open in the browser, not {scheme}:"
        ));
    }
    crate::browser::loadable(url)
        .ok_or_else(|| format!("`{url}` is not an address: give it whole, with no spaces"))
}

/// The call's fields carried out on `page`, in order: an address, a size, keys, a script, a wait
/// for text, a picture; then the page as it is.
fn run(page: &Page, args: &Value, url: Option<&str>) -> Result<Output, String> {
    let mut report = vec![];
    if let Some(url) = url {
        match page.open(url, url, None) {
            Ok(()) => report.push(format!("Opened {url}.")),
            Err(e) => report.push(format!("Opening {url} did not finish cleanly ({e}).")),
        }
    }
    if let Some(size) = text(args, "size") {
        match page.resize(size) {
            Ok(()) => report.push(format!("Page size {size}.")),
            Err(e) => report.push(format!("Page size not changed: {e}")),
        }
    }
    let driver = page.driver().map_err(|e| e.to_string())?;
    let keys = strings(args, "keys");
    if !keys.is_empty() {
        let delay = number(args, "key_delay_ms")
            .unwrap_or(DEFAULT_KEY_DELAY_MS)
            .min(MAX_KEY_DELAY_MS);
        match driver.press(&keys, delay) {
            Ok(()) => report.push(format!("Pressed {} keys.", keys.len())),
            Err(e) => report.push(format!("Pressing keys failed: {e}")),
        }
    }
    if let Some(source) = text(args, "script") {
        match driver.evaluate(source) {
            Ok(value) => report.push(format!("### Script result\n{value}")),
            Err(e) => report.push(format!("The script failed: {e}")),
        }
    }
    if let Some(wanted) = text(args, "wait_for") {
        let most = number(args, "yield_ms")
            .unwrap_or(DEFAULT_WAIT_MS)
            .min(MAX_WAIT_MS);
        report.push(wait_for(&driver, wanted, most));
    }
    let mut shot = None;
    if args.get("screenshot").and_then(Value::as_bool) == Some(true) {
        let selector = text(args, "selector");
        let of = selector.map_or_else(|| "the page".to_string(), |s| format!("`{s}`"));
        match driver.screenshot(selector) {
            Ok(taken) => {
                report.push(format!("Screenshot of {of}."));
                shot = Some(taken);
            }
            Err(e) => report.push(format!("No screenshot of {of}: {e}")),
        }
    }
    let mut state = page.is_open().then(|| outline(page));
    if let (Some(state), true) = (
        &mut state,
        args.get("read").and_then(Value::as_bool) == Some(true),
    ) {
        state.push_str(&read(
            page,
            number(args, "offset").map_or(0, |n| usize::try_from(n).unwrap_or(0)),
        ));
    }
    Ok(Output {
        report: report.join("\n\n"),
        state,
        shot,
    })
}

/// The page as the agent reads it: where it is, then its outline without refs.
fn outline(page: &Page) -> String {
    match page.observe() {
        Ok(view) => {
            let outline = without_refs(&view.outline);
            let shown: String = outline.chars().take(OUTLINE_LIMIT).collect();
            let more = if shown.len() < outline.len() {
                "\n... (more below: `read` or `script` reach it)"
            } else {
                ""
            };
            format!(
                "### Page (untrusted content)\n{} - {}\n{shown}{more}",
                view.title, view.location
            )
        }
        Err(e) => format!("### Page\nThe page could not be read: {e}"),
    }
}

fn read(page: &Page, offset: usize) -> String {
    match page
        .driver()
        .and_then(|driver| driver.read(offset, READ_LIMIT))
    {
        Ok(text) => {
            let end = text.offset + text.text.chars().count();
            let more = if end < text.total {
                format!("\n... more: read again with offset {end}")
            } else {
                String::new()
            };
            format!(
                "\n\n### Full text (characters {}-{end} of {})\n{}{more}",
                text.offset, text.total, text.text
            )
        }
        Err(e) => format!("\n\n### Full text\nThe text could not be read: {e}"),
    }
}

/// The outline as a person reads it: ` [ref=e12]` marks dropped.
fn without_refs(outline: &str) -> String {
    let mut out = String::with_capacity(outline.len());
    let mut rest = outline;
    while let Some((before, marked)) = rest.split_once(" [ref=") {
        out.push_str(before);
        rest = marked.split_once(']').map_or("", |(_, after)| after);
    }
    out.push_str(rest);
    out
}

/// Waits until the page's text shows `wanted`, at most `most` ms, polled in the page on a timer
/// (animation frames stop in a window in the background); says whether it came and when.
fn wait_for(driver: &impl Driver, wanted: &str, most: u64) -> String {
    let literal = Value::String(wanted.to_string());
    let source = format!(
        "const wanted = {literal}; const until = performance.now() + {most}; \
         const shows = () => (document.body?.innerText ?? '').includes(wanted); \
         while (!shows() && performance.now() < until) await sleep(100); shows()"
    );
    let started = Instant::now();
    let seconds = |ms: u128| format!("{}.{}s", ms / 1000, ms % 1000 / 100);
    match driver.evaluate(&source).as_deref() {
        Ok("true") => format!(
            "\"{wanted}\" appeared after {}",
            seconds(started.elapsed().as_millis())
        ),
        Ok(_) => format!(
            "\"{wanted}\" did not appear within {}: the page may still be working; wait again, or read what it shows",
            seconds(u128::from(most))
        ),
        Err(e) => format!("Waiting for \"{wanted}\" failed: {e}"),
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
