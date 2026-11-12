//! What may load in the window, and the per-run token that keeps a page from answering for the probe.

/// Only web pages load in the window; other schemes (files, apps, mail) are refused. `data:` is
/// out too: WebKit drops IPC from its opaque origin, so the probe could never answer there.
pub(super) fn allowed(url: &str) -> bool {
    ["http://", "https://", "about:", "blob:"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
}

/// `url` as the window can load it: an address the system's URL type takes, spelled so (a space
/// or a letter past ASCII encoded); `None` for what is not an address at all. WebKit's load of
/// an address the system cannot read panics on the main thread, ending the whole process, so
/// nothing reaches a load without passing here.
pub fn loadable(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    let web = matches!(parsed.scheme(), "http" | "https");
    if web && parsed.host_str().is_none_or(str::is_empty) {
        return None;
    }
    // What the person or the model wrote stays as written when it needs no spelling out.
    let plain = url.chars().all(|c| c.is_ascii_graphic());
    Some(match plain {
        true => url.to_string(),
        false => parsed.to_string(),
    })
}

/// 32 hex characters from the system's random source; unguessable to a page in one run.
pub(super) fn token() -> String {
    super::secret::word()
}

/// Whether `url` is an address another browser can open: a web page, or a file.
pub(super) fn outside(url: &str) -> bool {
    ["http://", "https://", "file://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
}

/// The file `url` names: the last part of its path, as written before it was spelled for an
/// address. None for an address whose path ends in no name (a site's root, a folder).
pub(super) fn file_named(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let last = parsed.path_segments()?.next_back()?.to_owned();
    let plain = percent_encoding::percent_decode_str(&last)
        .decode_utf8()
        .map_or_else(|_| last.clone(), std::borrow::Cow::into_owned);
    (!plain.is_empty()).then_some(plain)
}

/// Markup a page renders, which is text too: two ways to be opened in a tab.
const RENDERED: [&str; 3] = ["html", "htm", "svg"];

fn of_kind(path: &std::path::Path, kinds: &[&str]) -> bool {
    (path.extension().and_then(|kind| kind.to_str()))
        .is_some_and(|kind| kinds.contains(&kind.to_ascii_lowercase().as_str()))
}

/// Whether the file at `path` is a picture written as text: it is looked at far more than it
/// is written, so a pick shows it drawn, and its text is the second way.
pub fn drawn(path: &std::path::Path) -> bool {
    of_kind(path, &["svg"])
}

/// Whether the file at `path` is text a page renders: it has two ways to be opened in a tab.
pub(super) fn renders(path: &std::path::Path) -> bool {
    of_kind(path, &RENDERED)
}
