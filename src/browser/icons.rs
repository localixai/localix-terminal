//! The pictures of the system's menus the window opens (menu.rs draws them as template images,
//! in the menu's own colour): a folder, a new one, and each kind of tab, drawn as the strip draws
//! its own.

/// One picture: `paths`, stroked, on the strip's 16-point grid.
macro_rules! icon {
    ($paths:literal) => {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="black" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">"#,
            $paths,
            "</svg>"
        )
    };
}

pub(super) const FOLDER: &str = icon!(
    r#"<path d="M2.5 4.6c0-.6.4-1.1 1-1.1h2.8l1.3 1.4h4.9c.6 0 1 .5 1 1.1v5.9c0 .6-.4 1.1-1 1.1h-9c-.6 0-1-.5-1-1.1z"/>"#
);
pub(super) const FOLDER_PLUS: &str = icon!(
    r#"<path d="M2.5 4.6c0-.6.4-1.1 1-1.1h2.8l1.3 1.4h4.9c.6 0 1 .5 1 1.1v5.9c0 .6-.4 1.1-1 1.1h-9c-.6 0-1-.5-1-1.1zM8 7.3v3.4M6.3 9h3.4"/>"#
);
const WEB: &str = icon!(
    r#"<circle cx="8" cy="8" r="5.6"/><path d="M2.6 8h10.8M8 2.4c1.6 1.6 1.6 9.6 0 11.2M8 2.4c-1.6 1.6-1.6 9.6 0 11.2"/>"#
);
const TERM: &str = icon!(r#"<path d="m4 5.5 2.5 2.5L4 10.5M8.5 10.5H12"/>"#);
const BOARD: &str =
    icon!(r#"<rect x="2.5" y="3" width="11" height="10" rx="1.4"/><path d="M6.2 3v10M9.8 3v10"/>"#);
const SETTINGS: &str = icon!(
    r#"<path d="M6.71 3.17 7.02 1.47h1.96l.31 1.7 1.21.5 1.43-.98 1.38 1.38-.98 1.43.5 1.21 1.7.31v1.96l-1.7.31-.5 1.21.98 1.43-1.38 1.38-1.43-.98-1.21.5-.31 1.7H7.02l-.31-1.7-1.21-.5-1.43.98-1.38-1.38.98-1.43-.5-1.21-1.7-.31V7.02l1.7-.31.5-1.21-.98-1.43 1.38-1.38 1.43.98z"/><circle cx="8" cy="8" r="2.1"/>"#
);

/// A tab's picture by its kind (strip.rs `Key::kind`).
pub(super) fn of_kind(kind: &str) -> &'static str {
    match kind {
        "term" => TERM,
        "board" => BOARD,
        "settings" => SETTINGS,
        _ => WEB,
    }
}
