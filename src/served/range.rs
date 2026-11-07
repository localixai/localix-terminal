//! What a request's `Range` header asks of the file it names.

/// What a `Range` header asks of a file of `length` bytes.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Slice {
    /// All of it: no range was asked, or none this server follows (several, or not in bytes).
    Whole,
    /// These bytes, the first and the last.
    Part(u64, u64),
    /// A range that starts past the end.
    Refused,
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n` of a file of `length` bytes.
pub(super) fn slice(range: Option<&str>, length: u64) -> Slice {
    let Some((first, last)) = range
        .and_then(|range| range.trim().strip_prefix("bytes="))
        .and_then(|spec| spec.split_once('-'))
        .filter(|_| length > 0)
    else {
        return Slice::Whole;
    };
    let (first, last) = (first.trim(), last.trim());
    let (from, to) = (first.parse::<u64>().ok(), last.parse::<u64>().ok());
    let end = length - 1;
    match (from, to) {
        (Some(from), Some(to)) if from <= to => match from > end {
            true => Slice::Refused,
            false => Slice::Part(from, to.min(end)),
        },
        (Some(from), None) if last.is_empty() => match from > end {
            true => Slice::Refused,
            false => Slice::Part(from, end),
        },
        (None, Some(0)) if first.is_empty() => Slice::Refused,
        (None, Some(count)) if first.is_empty() => Slice::Part(length.saturating_sub(count), end),
        _ => Slice::Whole,
    }
}
