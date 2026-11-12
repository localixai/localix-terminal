//! What the app's own pages share, whatever draws them (the board, the Store, an editor, a
//! terminal): the script each runs before its own (`page/kit.js`: what a page posts, how it
//! builds an element, the system's menu), and how words and data are put into a page so that
//! none of them is read as markup or as script. A page's module joins the kit to its script
//! with `concat!`, so the two are one script under one hash.

use serde_json::Value;

/// `text` with what would be markup spelled out: a title, a name, a line of a page.
pub fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `data` as a script holds it: JSON is script as it is, but for the two line ends a script
/// may not hold.
pub fn json(data: &Value) -> String {
    data.to_string()
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// The SHA-384 of `script`, as a page's policy names the one script it runs.
#[cfg(test)]
pub fn digest(script: &str) -> String {
    use base64::Engine as _;
    use sha2::{Digest as _, Sha384};
    base64::engine::general_purpose::STANDARD.encode(Sha384::digest(script))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
