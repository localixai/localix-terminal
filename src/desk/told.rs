//! What a page told during a call, besides what it shows (a dialog and its answer, words it
//! announced, a field that did not keep what was typed), put in words after the call's report;
//! and the answer a call arms for a page's confirms and prompts, for as long as the call lasts.

use serde_json::Value;

use crate::browser::{Armed, Notice, WebviewDriver};

use super::Desk;
use super::args::text;

/// A tab armed to answer OK, for as long as this lives: then Cancel again.
pub(super) struct Answering(WebviewDriver);

impl Drop for Answering {
    fn drop(&mut self) {
        self.0.arm(None);
    }
}

/// How the call wants a page's confirms and prompts answered: OK, with `dialog_text` for a
/// prompt, when `dialog` is `accept`; Cancel when it is `dismiss` or not given.
pub(super) fn wanted(args: &Value) -> Result<Option<Armed>, String> {
    match text(args, "dialog") {
        None | Some("dismiss") => Ok(None),
        Some("accept") => Ok(Some(Armed {
            text: args
                .get("dialog_text")
                .and_then(Value::as_str)
                .map(String::from),
        })),
        Some(other) => Err(format!("`dialog` is accept or dismiss, not `{other}`")),
    }
}

impl Desk {
    /// Browser tab `id` answers OK while the returned guard lives, when the call asks for it.
    pub(super) fn answering(&self, id: u32, args: &Value) -> Result<Option<Answering>, String> {
        let Some(armed) = wanted(args)? else {
            return Ok(None);
        };
        let driver = self
            .page()?
            .pinned(id)
            .driver()
            .map_err(|e| e.to_string())?;
        driver.arm(Some(armed));
        Ok(Some(Answering(driver)))
    }

    /// Where the pages' notices stand now. Before the window opens, the start: a window keeps
    /// nothing from before it opened (a closed one forgets its tabs' notices), so all a tab of
    /// the call that opens it tells is this call's.
    pub(super) fn told_mark(&self) -> u64 {
        (self.page().ok())
            .and_then(|page| page.driver().ok())
            .map_or(0, |driver| driver.told_mark())
    }

    /// What browser tab `id` told after `mark`, in words; none when it told nothing.
    pub(super) fn told(&self, id: u32, mark: u64) -> Option<String> {
        let driver = self.page().ok()?.pinned(id).driver().ok()?;
        told_block(&driver.told_since(mark))
    }
}

/// `notices` as the agent reads them.
pub(super) fn told_block(notices: &[Notice]) -> Option<String> {
    if notices.is_empty() {
        return None;
    }
    let mut lines = vec!["### The page said (untrusted content)".to_string()];
    lines.extend(notices.iter().map(|notice| format!("- {notice}")));
    if notices.iter().any(Notice::declined) {
        lines.push(
            "A dialog answered Cancel did not go ahead: to answer OK, act again with \
             `dialog: \"accept\"` (and `dialog_text` for a prompt)."
                .into(),
        );
    }
    Some(lines.join("\n"))
}

#[cfg(test)]
#[path = "told_tests.rs"]
mod tests;
