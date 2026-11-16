//! What a tab of the strip is marked with besides its name and picture: the words of what waits
//! for the person in it, read from the Event Center's list (alerts.rs), what its terminal's
//! program says it is at work on (OSC 7501), which its icon shows as a still ring, and how many
//! panes its tab's zoom puts by.

use serde_json::{Value, json};

/// A terminal's program at work, by its own report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Working {
    /// How far, 0 to 100; none: not known.
    pub progress: Option<u8>,
}

/// What a tab is marked with.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(in crate::browser) struct Marks {
    /// The words of what waits for the person in it, when something does.
    pub waits: Option<String>,
    /// Its program at work, when it says so.
    pub working: Option<Working>,
    /// Its tab zoomed on it (or, the side's row of that tab, on one of its panes): how many of
    /// the tab's panes are put by.
    pub zoomed: Option<usize>,
}

/// `tab`, as the strip's page reads it, given its marks: none says nothing.
pub(super) fn mark(tab: &mut Value, marks: &Marks) {
    if let Some(waits) = &marks.waits {
        tab["waits"] = json!(waits);
    }
    if let Some(working) = marks.working {
        tab["working"] = json!({"progress": working.progress});
    }
    if let Some(hidden) = marks.zoomed {
        tab["zoomed"] = json!(hidden);
    }
}
