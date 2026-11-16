//! The terminals the app had no room for: the last one of each project is one quiet line there,
//! titled as a terminal is (its command, or `shell`), until a terminal opens in that project or
//! the person dismisses it. It leads nowhere: what would make room is a program the person ends.

use super::sources::Source;
use super::{Event, Kind};
use crate::desk::Desk;
use crate::desk::term::are_open;

/// The projects where a terminal did not open for want of room.
pub(super) struct Refusals;

impl Source for Refusals {
    fn gather(&self, desk: &Desk) -> Vec<Event> {
        (desk.room.refused().into_iter())
            .map(|(group, refused)| {
                let detail = format!(
                    "Not opened: {}, the most this machine holds",
                    are_open(refused.open)
                );
                Event::new(
                    format!("room:{group}:{}", refused.n),
                    Kind::Refused,
                    &group,
                    "",
                )
                .says(&refused.title, detail)
                .at(refused.at)
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "refused_tests.rs"]
mod tests;
