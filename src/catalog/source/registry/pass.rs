//! How far the registry has been read: a pass through its servers, page by page. The first
//! pass reads every server; each pass after reads only those updated since the last one
//! began. The mark it leaves says which pass, from when, and the page it is at, so a pass cut
//! short goes on where it was.

use serde::{Deserialize, Serialize};

/// A pass, as its mark keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Pass {
    /// What this pass asks for: the servers updated since then (RFC 3339), or all of them.
    pub(super) since: Option<String>,
    /// When this pass began (RFC 3339): what the next one asks from.
    pub(super) began: String,
    /// The page this pass goes on from; none when it is done, or has not read its first.
    pub(super) cursor: Option<String>,
    /// It is done: its last page was read.
    pub(super) done: bool,
}

/// How far a pass is begun before its clock says, so a server updated while the registry's
/// clock and this one's differ is asked again rather than missed.
const MARGIN: u64 = 10 * 60;

impl Pass {
    /// The pass to read now, after the one `mark` left (none: the first), at `now` (seconds of
    /// the epoch): the same pass where it was cut short, else a new one from when the last
    /// began.
    pub(super) fn after(mark: Option<&str>, now: u64) -> Self {
        let began = rfc3339(now.saturating_sub(MARGIN));
        let last = mark.and_then(|mark| serde_json::from_str::<Self>(mark).ok());
        match last {
            Some(last) if !last.done => last,
            Some(last) => Self {
                since: Some(last.began),
                began,
                cursor: None,
                done: false,
            },
            None => Self {
                since: None,
                began,
                cursor: None,
                done: false,
            },
        }
    }

    /// This pass once a page was read whose next page is `next`: on there, or done.
    #[must_use]
    pub(super) fn read(self, next: Option<String>) -> Self {
        let done = next.is_none();
        Self {
            cursor: next,
            done,
            ..self
        }
    }

    pub(super) fn mark(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// `seconds` of the epoch as an RFC 3339 time in UTC: `2026-11-12T13:18:20Z`.
pub(super) fn rfc3339(seconds: u64) -> String {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil(days);
    let (hour, minute, second) = (rest / 3600, rest % 3600 / 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// The year, month and day `days` after 1970-01-01, in the proleptic Gregorian calendar.
fn civil(days: u64) -> (u64, u64, u64) {
    // Counted in eras of 400 years from 0000-03-01, so a leap day ends its year.
    let days = days + 719_468;
    let era = days / 146_097;
    let of_era = days % 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[path = "pass_tests.rs"]
mod tests;
