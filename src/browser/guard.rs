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

/// The kinds of file a page shows as they are: markup it renders, a picture, a document.
const SHOWN: [&str; 11] = [
    "html", "htm", "svg", "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "pdf",
];

/// Of them, the ones that are text: an editor takes them too, and a page renders them.
const RENDERED: [&str; 3] = ["html", "htm", "svg"];

fn of_kind(path: &std::path::Path, kinds: &[&str]) -> bool {
    (path.extension().and_then(|kind| kind.to_str()))
        .is_some_and(|kind| kinds.contains(&kind.to_ascii_lowercase().as_str()))
}

/// Whether a page shows the file at `path` as it is.
pub fn shows(path: &std::path::Path) -> bool {
    of_kind(path, &SHOWN)
}

/// Whether the file at `path` is text a page renders: it has two ways to be opened in a tab.
pub(super) fn renders(path: &std::path::Path) -> bool {
    of_kind(path, &RENDERED)
}
