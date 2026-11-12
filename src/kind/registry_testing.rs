//! The kinds under test: each test's thread has its own list, empty until it lists some, so no
//! test sees the kinds of another.

use std::sync::{Arc, Mutex};

use super::Kind;

thread_local! {
    static KINDS: Mutex<Vec<Arc<dyn Kind>>> = const { Mutex::new(Vec::new()) };
}

pub(super) fn install(kinds: Vec<Arc<dyn Kind>>) {
    KINDS.with(|held| *crate::lock(held) = kinds);
}

pub(super) fn all() -> Vec<Arc<dyn Kind>> {
    KINDS.with(|held| crate::lock(held).clone())
}
