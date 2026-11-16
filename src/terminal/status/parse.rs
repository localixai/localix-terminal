//! A report of the program status protocol read by its grammar: `key=value` pairs split by `:`,
//! a pair that breaks the grammar passed over, the last of a key kept. What would change its
//! meaning drops the whole report rather than read it as something else: a state not known, an
//! id that breaks the rules (it never becomes the root), a text that is no base64 of plain words.

use base64::Engine;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};

use super::{Asks, Doing};

/// The longest report, as the sequence's body after `7501;`.
pub(in crate::terminal) const BODY_MOST: usize = 4096 - "\x1b]7501;\x1b\\".len();
const KEY_MOST: usize = 16;
const APP_MOST: usize = 32;
const ID_MOST: usize = 128;
const ID_PART_MOST: usize = 32;
const ID_DEPTH: usize = 8;
/// A title's and a message's most, as base64 and as the words it holds.
const TITLE_MOST: (usize, usize) = (256, 192);
const MSG_MOST: (usize, usize) = (2732, 2048);

/// What a sequence of the protocol says.
#[derive(Debug)]
pub(in crate::terminal) enum Said {
    /// Which states the terminal reads: answered with `REPLY`.
    Query,
    Report(Report),
}

/// One report, as read.
#[derive(Debug, Default, PartialEq, Eq)]
pub(in crate::terminal) struct Report {
    /// The record it is of; empty: the root.
    pub(in crate::terminal) id: String,
    /// None: the record and its children are cleared.
    pub(in crate::terminal) state: Option<Doing>,
    pub(in crate::terminal) asks: Option<Asks>,
    pub(in crate::terminal) app: Option<String>,
    pub(in crate::terminal) title: Option<String>,
    pub(in crate::terminal) msg: Option<String>,
    pub(in crate::terminal) progress: Option<u8>,
}

/// A sequence's body, after `7501;`. None: it says nothing to keep.
pub(in crate::terminal) fn read(body: &str) -> Option<Said> {
    if body.starts_with('?') {
        return Some(Said::Query);
    }
    if body.len() > BODY_MOST {
        return None;
    }
    let mut pairs: Vec<(&str, &str)> = vec![];
    for pair in body.split(':') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let key_ok = !key.is_empty()
            && key.len() <= KEY_MOST
            && key.bytes().all(|byte| byte.is_ascii_lowercase());
        let value_ok =
            (value.bytes()).all(|byte| byte.is_ascii_alphanumeric() || b"_.,+/=-".contains(&byte));
        if key_ok && value_ok {
            pairs.retain(|(had, _)| *had != key);
            pairs.push((key, value));
        }
    }
    let value = |key: &str| pairs.iter().find(|(had, _)| *had == key).map(|(_, v)| *v);
    let state = match value("state")? {
        "idle" => Some(Doing::Idle),
        "working" => Some(Doing::Working),
        "done" => Some(Doing::Done),
        "blocked" => Some(Doing::Blocked),
        "error" => Some(Doing::Error),
        "clear" => None,
        _ => return None,
    };
    let id = match value("id") {
        Some(id) if !id_ok(id) => return None,
        id => id.unwrap_or_default().to_string(),
    };
    let asks = match (state, value("kind")) {
        (Some(Doing::Blocked), Some("permission")) => Some(Asks::Permission),
        (Some(Doing::Blocked), Some("question")) => Some(Asks::Question),
        (Some(Doing::Blocked), Some("auth")) => Some(Asks::Auth),
        _ => None,
    };
    let progress = (value("progress").and_then(|progress| progress.parse::<u8>().ok()))
        .filter(|progress| *progress <= 100)
        .filter(|_| matches!(state, Some(Doing::Working | Doing::Blocked)));
    let app =
        (value("app").filter(|app| !app.is_empty() && app.len() <= APP_MOST)).map(str::to_string);
    let title = match value("title") {
        Some(encoded) => Some(words(encoded, TITLE_MOST)?).filter(|words| !words.is_empty()),
        None => None,
    };
    let msg = match value("msg") {
        Some(encoded) => Some(words(encoded, MSG_MOST)?).filter(|words| !words.is_empty()),
        None => None,
    };
    Some(Said::Report(Report {
        id,
        state,
        asks,
        app,
        title,
        msg,
        progress,
    }))
}

/// Whether `id` is a path of 1 to 8 parts of `[A-Za-z0-9_.+-]{1,32}`, at most 128 bytes.
fn id_ok(id: &str) -> bool {
    let parts: Vec<&str> = id.split('/').collect();
    id.len() <= ID_MOST
        && parts.len() <= ID_DEPTH
        && parts.iter().all(|part| {
            (1..=ID_PART_MOST).contains(&part.len())
                && (part.bytes())
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.+-".contains(&byte))
        })
}

/// Base64 that either alphabet writes, padded or not.
const LOOSE: GeneralPurposeConfig =
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
const STANDARD: GeneralPurpose = GeneralPurpose::new(&base64::alphabet::STANDARD, LOOSE);
const URL_SAFE: GeneralPurpose = GeneralPurpose::new(&base64::alphabet::URL_SAFE, LOOSE);

/// The words `encoded` holds, plain (empty when there are none); None when it is no base64 of
/// UTF-8 within `most`, or holds a control character.
fn words(encoded: &str, (encoded_most, most): (usize, usize)) -> Option<String> {
    if encoded.len() > encoded_most {
        return None;
    }
    let engine = if encoded.contains(['-', '_']) {
        URL_SAFE
    } else {
        STANDARD
    };
    let bytes = engine.decode(encoded).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    if text.len() > most || text.chars().any(char::is_control) {
        return None;
    }
    // What would turn the line around or hide in it (bidi controls, zero widths) is taken out.
    let shown: String = text.chars().filter(|ch| !hidden(*ch)).collect();
    Some(shown.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn hidden(ch: char) -> bool {
    matches!(ch, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}' | '\u{2060}')
}
