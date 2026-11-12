//! The kinds as the running app holds them: listed once, before the window and the desk are made.

use std::sync::{Arc, OnceLock};

use super::Kind;

static KINDS: OnceLock<Vec<Arc<dyn Kind>>> = OnceLock::new();

/// The first list given stays.
pub(super) fn install(kinds: Vec<Arc<dyn Kind>>) {
    let _ = KINDS.set(kinds);
}

pub(super) fn all() -> Vec<Arc<dyn Kind>> {
    KINDS.get().cloned().unwrap_or_default()
}
