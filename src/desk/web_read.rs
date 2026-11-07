//! Read and label the page's untrusted outline and full text, in the probe's UTF-16 units.

use crate::browser::{Driver, Page};
use crate::view::View;

/// Characters of the page outline read after each call, and of text one `read` returns.
const OUTLINE_LIMIT: usize = 6000;
const READ_LIMIT: usize = 8000;
/// Characters of a page's title and address shown with its outline: a page writes both.
pub(super) const TITLE_LIMIT: usize = 200;
const LOCATION_LIMIT: usize = 500;

/// The page as the agent reads it: where it is, then its outline with each control's ref.
pub(super) fn outline(page: &Page) -> String {
    match page.observe() {
        Ok(view) => {
            let outline = short_refs(&view.outline);
            let shown: String = outline.chars().take(OUTLINE_LIMIT).collect();
            let more = if shown.len() < outline.len() {
                "\n... (more below: `read` or `script` reach it)"
            } else {
                ""
            };
            format!(
                "### Page (untrusted content)\n{} - {}\n{shown}{more}",
                clipped(&view.title, TITLE_LIMIT),
                clipped(&view.location, LOCATION_LIMIT)
            )
        }
        Err(e) => format!("### Page\nThe page could not be read: {e}"),
    }
}

/// `text` cut to `most` characters, with `…` where it was.
pub(super) fn clipped(text: &str, most: usize) -> String {
    let mut kept: String = text.chars().take(most).collect();
    if kept.len() < text.len() {
        kept.push('…');
    }
    kept
}

/// Where the window of text that starts at `offset` ends, and the next one starts. The probe
/// counts in UTF-16 units, as a script does, so an emoji is two.
pub(super) fn text_end(offset: usize, text: &str) -> usize {
    offset + text.encode_utf16().count()
}

pub(super) fn read(page: &Page, offset: usize) -> String {
    match page
        .driver()
        .and_then(|driver| driver.read(offset, READ_LIMIT))
    {
        Ok(text) => {
            let end = text_end(text.offset, &text.text);
            let more = if end < text.total {
                format!("\n... more: read again with offset {end}")
            } else {
                String::new()
            };
            format!(
                "\n\n### Full text (untrusted content, characters {}-{end} of {})\n{}{more}",
                text.offset, text.total, text.text
            )
        }
        Err(e) => format!("\n\n### Full text\nThe text could not be read: {e}"),
    }
}

/// The outline as the agent reads it: each control's ` [ref=e12]` as ` [e12]`, what `click`,
/// `fill` and `select` name it by.
pub(super) fn short_refs(outline: &str) -> String {
    outline.replace(" [ref=", " [")
}
