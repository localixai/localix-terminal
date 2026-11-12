//! The pictures of the system's menus the window opens (menu.rs draws them as template images,
//! in the menu's own colour): a folder, a new one, and each kind of tab, drawn as the strip draws
//! its own. Lines that meet are one path, painted once: a shape laid over another shows at
//! every joint in an ink that is not opaque.

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
    r#"<path d="M8 2.4a5.6 5.6 0 1 0 0 11.2a5.6 5.6 0 1 0 0 -11.2zM2.4 8h11.2M8 2.4c1.6 1.6 1.6 9.6 0 11.2M8 2.4c-1.6 1.6-1.6 9.6 0 11.2"/>"#
);
const TERM: &str = icon!(r#"<path d="m4 5.5 2.5 2.5L4 10.5M8.5 10.5H12"/>"#);
const BOARD: &str = icon!(
    r#"<path d="M3.9 3h8.2a1.4 1.4 0 0 1 1.4 1.4v7.2a1.4 1.4 0 0 1 -1.4 1.4h-8.2a1.4 1.4 0 0 1 -1.4 -1.4v-7.2a1.4 1.4 0 0 1 1.4 -1.4zM6.2 3v10M9.8 3v10"/>"#
);
const FILE: &str = icon!(
    r#"<path d="M4.5 2.5h4.6l2.9 2.9v7c0 .6-.5 1.1-1.1 1.1H4.5c-.6 0-1-.5-1-1.1V3.6c0-.6.4-1.1 1-1.1zM9 2.6v2.9h2.9"/>"#
);
const SETTINGS: &str = icon!(
    r#"<path d="M6.45 3.88L6.52 2.19A6 6 0 0 1 9.48 2.19L9.55 3.88A4.4 4.4 0 0 1 10.79 4.6L12.29 3.81A6 6 0 0 1 13.78 6.38L12.34 7.28A4.4 4.4 0 0 1 12.34 8.72L13.78 9.62A6 6 0 0 1 12.29 12.19L10.79 11.4A4.4 4.4 0 0 1 9.55 12.12L9.48 13.81A6 6 0 0 1 6.52 13.81L6.45 12.12A4.4 4.4 0 0 1 5.21 11.4L3.71 12.19A6 6 0 0 1 2.22 9.62L3.66 8.72A4.4 4.4 0 0 1 3.66 7.28L2.22 6.38A6 6 0 0 1 3.71 3.81L5.21 4.6A4.4 4.4 0 0 1 6.45 3.88z"/><circle cx="8" cy="8" r="1.9"/>"#
);

/// A tab's picture by its kind (strip.rs `Key::kind`).
pub(super) fn of_kind(kind: &str) -> &'static str {
    match kind {
        "term" => TERM,
        "board" => BOARD,
        "file" => FILE,
        "settings" => SETTINGS,
        "web" => WEB,
        made => of_made(made).unwrap_or(WEB),
    }
}

/// The picture of the kind that draws itself named `kind`, made of its glyph once and kept.
fn of_made(kind: &str) -> Option<&'static str> {
    static MADE: std::sync::Mutex<Vec<(&'static str, &'static str)>> =
        std::sync::Mutex::new(Vec::new());
    let about = crate::kind::named(kind)?.about();
    let mut made = crate::lock(&MADE);
    if let Some((_, picture)) = made.iter().find(|(id, _)| *id == about.id) {
        return Some(picture);
    }
    let glyph: String = (about.glyph.chars())
        .filter(|c| c.is_ascii_alphanumeric() || " .,-".contains(*c))
        .collect();
    let picture: &'static str =
        Box::leak(format!(icon!(r#"<path d="{}"/>"#), glyph).into_boxed_str());
    made.push((about.id, picture));
    Some(picture)
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;
