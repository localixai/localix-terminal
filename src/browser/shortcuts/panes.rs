//! The shortcuts of the panes of the tab shown (docs/windows.md), rows of the one catalog
//! (catalog.rs `every`): a new terminal beside the one in use, the keys to another pane, the
//! pane in use zoomed, a divider moved, the tab made even, a pane to a tab of its own. Their
//! group is a menu of the menu bar and a section of Settings.

use super::catalog::{Shortcut, at};

/// The group of the panes' shortcuts.
const PANE: &str = "Pane";
/// The far steps of a divider: Shift with the step's chord.
const FAR_LEFT: &str = "ctrl+shift+cmd+left";
const FAR_RIGHT: &str = "ctrl+shift+cmd+right";
const FAR_UP: &str = "ctrl+shift+cmd+up";
const FAR_DOWN: &str = "ctrl+shift+cmd+down";

/// The panes' shortcuts, in the order the menu and Settings list them. The keys are those the
/// Mac's terminals split and move by; ⌃⌘ and an arrow is no key the system keeps for itself
/// (its Spaces and Mission Control take ⌃ and an arrow, its tiling Fn ⌃ and an arrow).
pub(super) const PANES: [Shortcut; 19] = [
    at(
        "split-right",
        "Split Right",
        PANE,
        "cmd+d",
        "key split right",
    ),
    at(
        "split-down",
        "Split Down",
        PANE,
        "cmd+shift+d",
        "key split down",
    ),
    at(
        "pane-left",
        "Select Pane Left",
        PANE,
        "alt+cmd+left",
        "key pane left",
    ),
    at(
        "pane-right",
        "Select Pane Right",
        PANE,
        "alt+cmd+right",
        "key pane right",
    ),
    at(
        "pane-up",
        "Select Pane Above",
        PANE,
        "alt+cmd+up",
        "key pane up",
    ),
    at(
        "pane-down",
        "Select Pane Below",
        PANE,
        "alt+cmd+down",
        "key pane down",
    ),
    at("pane-next", "Next Pane", PANE, "alt+cmd+]", "key pane next"),
    at(
        "pane-previous",
        "Previous Pane",
        PANE,
        "alt+cmd+[",
        "key pane previous",
    ),
    at("zoom", "Zoom Pane", PANE, "cmd+shift+enter", "key zoom"),
    at(
        "resize-left",
        "Move Divider Left",
        PANE,
        "ctrl+cmd+left",
        "key resize left",
    ),
    at(
        "resize-right",
        "Move Divider Right",
        PANE,
        "ctrl+cmd+right",
        "key resize right",
    ),
    at(
        "resize-up",
        "Move Divider Up",
        PANE,
        "ctrl+cmd+up",
        "key resize up",
    ),
    at(
        "resize-down",
        "Move Divider Down",
        PANE,
        "ctrl+cmd+down",
        "key resize down",
    ),
    at(
        "resize-far-left",
        "Move Divider Far Left",
        PANE,
        FAR_LEFT,
        "key resize far left",
    ),
    at(
        "resize-far-right",
        "Move Divider Far Right",
        PANE,
        FAR_RIGHT,
        "key resize far right",
    ),
    at(
        "resize-far-up",
        "Move Divider Far Up",
        PANE,
        FAR_UP,
        "key resize far up",
    ),
    at(
        "resize-far-down",
        "Move Divider Far Down",
        PANE,
        FAR_DOWN,
        "key resize far down",
    ),
    at(
        "equalize",
        "Equalize Panes",
        PANE,
        "ctrl+cmd+=",
        "key equalize",
    ),
    // No key of its own: Split View (⇧⌘\) does it in a tab of several panes.
    at("pane-out", "Move Pane to New Tab", PANE, "", "key pane out"),
];
