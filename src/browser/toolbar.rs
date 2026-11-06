//! The browser's own controls, shown beside the page like a real browser's toolbar: back and
//! reload, acted on like any control, by ref; the page never sees them. The page size (sizes.rs)
//! is the agent's `size`, not a control.

use crate::view::Element;

pub(crate) const BACK: &str = "back";
pub(crate) const RELOAD: &str = "reload";

/// Whether `reference` names a toolbar control rather than something on the page.
pub(crate) fn owns(reference: &str) -> bool {
    [BACK, RELOAD].contains(&reference)
}

/// The toolbar's controls.
pub(crate) fn controls() -> Vec<Element> {
    let button = |reference: &str, name: &str| Element {
        reference: reference.into(),
        role: "button".into(),
        name: name.into(),
        value: None,
        states: vec![],
        editable: false,
        dialog: false,
        options: vec![],
        form: None,
    };
    vec![button(BACK, "Back"), button(RELOAD, "Reload")]
}

/// The toolbar as outline lines, before the page's own.
pub(crate) fn outline(controls: &[Element]) -> String {
    let mut lines = vec!["- toolbar".to_string()];
    for control in controls {
        lines.push(format!(
            "  - {} [ref={}]",
            control.describe(),
            control.reference
        ));
    }
    lines.join("\n")
}

/// The control a key stands for: a browser takes its own shortcuts before the page sees them, and
/// Ctrl+R, Cmd+R and F5 (Shift with them too) reload it.
pub(crate) fn shortcut(key: &str) -> Option<&'static str> {
    let parts: Vec<String> = key.split('+').map(str::to_ascii_lowercase).collect();
    let (last, held) = parts.split_last()?;
    let command =
        |part: &String| ["ctrl", "control", "meta", "cmd", "command"].contains(&part.as_str());
    if !held.iter().all(|part| command(part) || part == "shift") {
        return None;
    }
    match last.as_str() {
        "f5" => Some(RELOAD),
        "r" if held.iter().any(command) => Some(RELOAD),
        _ => None,
    }
}

#[cfg(test)]
#[path = "toolbar_tests.rs"]
mod tests;
