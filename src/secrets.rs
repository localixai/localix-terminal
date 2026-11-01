//! Secrets an agent may have typed without ever seeing them: a password saved once with
//! `llx-desk secret NAME < FILE`, kept 0600 in the config directory beside the OpenRouter key,
//! and named in a call as `{{secret:NAME}}` (in a goal's `values`, or a terminal's `input`). The
//! daemon puts the value in where it is typed, and every answer it gives shows `{{secret:NAME}}`
//! wherever the value would be. Nothing prints a saved value.

use std::fmt::Write as _;
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How a call names a secret: `{{secret:NAME}}`.
const OPEN: &str = "{{secret:";
const CLOSE: &str = "}}";
/// A value shorter than this would be found in too much ordinary text to be told apart.
pub const MIN_LEN: usize = 4;

/// Where the secrets are: `secrets` beside the saved OpenRouter key.
pub fn dir() -> Option<PathBuf> {
    crate::key::path().and_then(|key| key.parent().map(|dir| dir.join("secrets")))
}

/// A name is letters, digits, `_` and `-`, at most 64 of them.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `{{secret:NAME}}`.
pub fn reference(name: &str) -> String {
    format!("{OPEN}{name}{CLOSE}")
}

/// The names `text` refers to, in order, each once.
pub fn named(text: &str) -> Vec<String> {
    let mut names: Vec<String> = vec![];
    let mut rest = text;
    while let Some((_, after)) = rest.split_once(OPEN) {
        let Some((name, tail)) = after.split_once(CLOSE) else {
            break;
        };
        if valid_name(name) && !names.iter().any(|known| known == name) {
            names.push(name.to_string());
        }
        rest = tail;
    }
    names
}

/// `value` saved as secret `name` in `dir`, readable by its owner alone, in a directory only they
/// open; its path.
pub fn save_in(dir: &Path, name: &str, value: &str) -> io::Result<PathBuf> {
    let invalid = |why: &str| io::Error::new(io::ErrorKind::InvalidInput, why.to_string());
    if !valid_name(name) {
        return Err(invalid(
            "a secret's name is letters, digits, _ and -, at most 64",
        ));
    }
    let value = value.trim_end_matches(['\n', '\r']);
    if value.chars().count() < MIN_LEN {
        return Err(invalid("a secret is at least 4 characters"));
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let path = dir.join(name);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    io::Write::write_all(&mut file, value.as_bytes())?;
    Ok(path)
}

/// Secret `name` in `dir`, if it is saved.
pub fn read_in(dir: &Path, name: &str) -> Option<String> {
    if !valid_name(name) {
        return None;
    }
    let value = std::fs::read_to_string(dir.join(name)).ok()?;
    (value.chars().count() >= MIN_LEN).then_some(value)
}

/// The names saved in `dir`, sorted.
pub fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = (std::fs::read_dir(dir).into_iter().flatten().flatten())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| valid_name(name))
        .collect();
    names.sort_unstable();
    names
}

/// Secret `name` removed from `dir`; whether there was one.
pub fn forget_in(dir: &Path, name: &str) -> io::Result<bool> {
    if !valid_name(name) {
        return Ok(false);
    }
    match std::fs::remove_file(dir.join(name)) {
        Ok(()) => Ok(true),
        Err(why) if why.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(why) => Err(why),
    }
}

/// The secrets this daemon put in somewhere, by name: every answer is masked with them.
#[derive(Default)]
pub struct Vault {
    known: Mutex<Vec<(String, String)>>,
}

impl Vault {
    /// `text` with each secret it names put in, read from `dir`; the first name not saved, else.
    pub fn fill_in(&self, dir: Option<&Path>, text: &str) -> Result<String, String> {
        let mut filled = text.to_string();
        for name in named(text) {
            let value = dir.and_then(|dir| read_in(dir, &name)).ok_or_else(|| {
                format!(
                    "no secret {name}: the person saves it once with `llx-desk secret {name} < FILE`"
                )
            })?;
            filled = filled.replace(&reference(&name), &value);
            let mut known = crate::lock(&self.known);
            known.retain(|(known, _)| known != &name);
            known.push((name, value));
        }
        Ok(filled)
    }

    /// `text` with every known secret's value shown as its reference: as typed, and in the forms
    /// a program writes it in (`forms`).
    pub fn mask(&self, text: &str) -> String {
        masked(&crate::lock(&self.known), text)
    }

    /// The known secrets, for a goal's view to put in and mask.
    pub fn known(&self) -> Vec<(String, String)> {
        crate::lock(&self.known).clone()
    }
}

/// `text` with each of `known` (name, value) shown as its reference, in every form of it. One
/// pass, the longest form first wherever one starts, and a reference already in the text kept
/// whole: a secret's value inside another's reference (`PASS` in `{{secret:PASSWORD}}`) is
/// never a match.
pub(crate) fn masked(known: &[(String, String)], text: &str) -> String {
    let mut all: Vec<(String, String)> = (known.iter())
        .flat_map(|(name, value)| forms(value).into_iter().map(|form| (form, reference(name))))
        .collect();
    all.sort_by_key(|(form, _)| std::cmp::Reverse(form.len()));
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let kept = (rest.strip_prefix(OPEN))
            .and_then(|after| after.split_once(CLOSE))
            .filter(|(name, _)| valid_name(name))
            .map(|(name, _)| OPEN.len() + name.len() + CLOSE.len());
        let found = kept.map(|n| (n, None)).or_else(|| {
            (all.iter())
                .find(|(form, _)| rest.starts_with(form.as_str()))
                .map(|(form, shown)| (form.len(), Some(shown)))
        });
        match found.and_then(|(n, shown)| Some((rest.split_at_checked(n)?, shown))) {
            Some(((matched, after), shown)) => {
                out.push_str(shown.map_or(matched, String::as_str));
                rest = after;
            }
            None => {
                out.push(c);
                rest = rest.get(c.len_utf8()..).unwrap_or_default();
            }
        }
    }
    out
}

/// `value` and the forms a program writes it in: escaped in a form (a space as `+`) or by a
/// page's script (`encodeURIComponent`: a space as `%20`, `!` kept), in JSON, and in base64,
/// plain or for an address, its padding left out (an `=` after it stays in the text). The
/// longest first, so a form is never cut by a shorter one inside it.
fn forms(value: &str) -> Vec<String> {
    use base64::Engine;
    use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
    let json = serde_json::to_string(value).unwrap_or_default();
    let json = (json.strip_prefix('"'))
        .and_then(|json| json.strip_suffix('"'))
        .unwrap_or_default();
    let mut forms = vec![
        value.to_string(),
        url::form_urlencoded::byte_serialize(value.as_bytes()).collect(),
        uri_component(value),
        json.to_string(),
        STANDARD_NO_PAD.encode(value),
        URL_SAFE_NO_PAD.encode(value),
    ];
    forms.sort_by_key(|form| std::cmp::Reverse(form.len()));
    forms.dedup();
    forms.retain(|form| !form.is_empty());
    forms
}

/// `value` as a page's `encodeURIComponent` writes it.
fn uri_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;
