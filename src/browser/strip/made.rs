//! The tabs of the kinds that draw themselves (`crate::kind`) among the strip's: their panels'
//! ids, and what the strip's page is told of the kinds.

use serde_json::json;

/// Panels past this id are tabs of the kinds that draw themselves (`crate::kind`): each kind has
/// [`MADE_EACH`] ids of its own, in the order the kinds are listed.
pub(super) const MADE: u32 = 1 << 26;
/// How many tabs a kind that draws itself numbers.
const MADE_EACH: u32 = 1 << 16;

/// The panel of tab `n` of the kind at place `at` among the kinds.
pub fn made(at: usize, n: u32) -> Option<u32> {
    let at = u32::try_from(at).ok()?;
    (n > 0 && n < MADE_EACH)
        .then(|| MADE.checked_add(at.checked_mul(MADE_EACH)?)?.checked_add(n))
        .flatten()
}

/// What panel `id` is of, when it is of a kind that draws itself: the kind, and the tab's number.
pub(super) fn made_of(id: u32) -> Option<(&'static crate::kind::About, u32)> {
    let past = id.checked_sub(MADE)?;
    let kinds = crate::kind::abouts();
    let about = kinds.get(usize::try_from(past / MADE_EACH).ok()?)?;
    Some((about, past % MADE_EACH))
}

/// The key of the tab whose panel is `id`: its kind's letter and its number.
pub(super) fn key(id: u32) -> String {
    match made_of(id) {
        Some((about, n)) => format!("{}{n}", about.letter),
        None => format!("t{id}"),
    }
}

/// The kinds that draw themselves, in the place the page reads them from: data, never script.
pub(super) fn kinds() -> String {
    let kinds: Vec<_> = (crate::kind::abouts().into_iter())
        .map(|about| {
            json!({"id": about.id, "title": about.title, "holds": about.holds,
                "glyph": about.glyph, "key": about.keyed()})
        })
        .collect();
    let kinds = serde_json::Value::Array(kinds).to_string();
    format!("id=\"kinds\">{}<", kinds.replace('<', "\\u003c"))
}
