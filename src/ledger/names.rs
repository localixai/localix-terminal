//! The names of the kinds that draw themselves, as the ledger read them from its rows.

/// `name` for as long as the process runs: a kind's name read from a row, the same one each
/// time it is read. There are as many as there are kinds.
pub(super) fn known(name: &str) -> &'static str {
    static NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut names = crate::lock(&NAMES);
    if let Some(known) = names.iter().find(|known| **known == name) {
        return known;
    }
    let kept: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(kept);
    kept
}
