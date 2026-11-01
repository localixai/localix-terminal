//! Credentials known by their shape, not by a saved value: a provider's key (`sk-…`, `ghp_…`,
//! `AKIA…`), a bearer token, a password in an address, a value assigned to a name that says it is
//! a credential (`OPENAI_API_KEY=…`, `"password": "…"`). Masked in what a goal's decision model
//! is sent, which a page or a terminal can fill with what it shows. A word that is only a word is
//! never masked: a false alarm costs the goal its view, a rare miss costs less.

/// What stands where a credential was.
pub const MASK: &str = "[credential]";

/// Prefixes that start a provider's key; the rest must look random.
const PREFIXES: [&str; 24] = [
    "sk-",
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "pk_live_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxs-",
    "glpat-",
    "AIza",
    "ya29.",
    "whsec_",
    "shpat_",
    "hf_",
    "npm_",
    "pypi-",
    "sq0atp-",
];
/// Words in a name that say its value is a credential.
const NAMES: [&str; 11] = [
    "apikey",
    "secretkey",
    "accesskey",
    "accesstoken",
    "authtoken",
    "refreshtoken",
    "privatekey",
    "clientsecret",
    "password",
    "passwd",
    "secret",
];

/// Characters that end a word: spaces, quotes and brackets.
fn boundary(c: char) -> bool {
    c.is_whitespace() || "\"'`,;()[]{}<>|".contains(c)
}

/// A key's random part: long enough, of key characters, with letters and digits.
fn random(body: &str) -> bool {
    body.chars().count() >= 16
        && body
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        && body.chars().any(|c| c.is_ascii_digit())
        && body.chars().any(|c| c.is_ascii_alphabetic())
}

fn provider_key(word: &str) -> bool {
    let aws = (word.starts_with("AKIA") || word.starts_with("ASIA"))
        && word.chars().count() == 20
        && word
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    aws || PREFIXES.iter().any(|prefix| {
        word.strip_prefix(prefix)
            .is_some_and(|body| random(body) && body.chars().count() >= 20)
    })
}

/// A name that says its value is a credential: `MY_API_KEY`, `userAccessToken`, `password`.
fn credential_name(name: &str) -> bool {
    let squeezed: String = (name.chars())
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    !squeezed.is_empty() && NAMES.iter().any(|word| squeezed.contains(word))
}

/// A value assigned to such a name that looks like a secret, not a word: digits or signs in it,
/// or long.
fn secret_value(value: &str) -> bool {
    let n = value.chars().count();
    n >= 6
        && !value.contains(MASK)
        && (n >= 16 || value.chars().any(|c| !c.is_alphabetic()))
        && !value.chars().all(|c| c == '*' || c == '•')
}

/// `word` with what in it is a credential masked; `after_name` when the word before named one
/// (`"password": "x"`, `Bearer x`).
fn masked_word(word: &str, after_name: bool) -> String {
    if provider_key(word) || (after_name && secret_value(word)) {
        return MASK.to_string();
    }
    // A password in an address: scheme://user:password@host.
    if let Some((scheme, rest)) = word.split_once("://")
        && let Some((userinfo, host)) = rest.split_once('@')
        && let Some((user, password)) = userinfo.split_once(':')
        && !password.is_empty()
    {
        return assigned_masked(&format!("{scheme}://{user}:{MASK}@{host}"));
    }
    assigned_masked(word)
}

/// `word` with each assignment in it masked: `NAME=value`, `NAME:value`, and every `name=value`
/// of an address's query or a form's body (`?user=ada&password=…`), not only the first. An `&`
/// that starts no `name=` is a value's own (`Tr0ub4dor&3`).
fn assigned_masked(word: &str) -> String {
    let (path, query) = match word.split_once('?') {
        Some((path, query)) => (Some(path), query),
        None => (None, word),
    };
    let mut params: Vec<String> = vec![];
    for param in query.split('&') {
        match params.last_mut() {
            Some(last) if !param.contains('=') => {
                last.push('&');
                last.push_str(param);
            }
            _ => params.push(param.to_string()),
        }
    }
    let query = (params.iter())
        .map(|param| assignment(param))
        .collect::<Vec<_>>()
        .join("&");
    match path {
        Some(path) => format!("{}?{query}", assignment(path)),
        None => query,
    }
}

/// `part` masked when it assigns a secret-looking value to a credential's name.
fn assignment(part: &str) -> String {
    for sign in ['=', ':'] {
        if let Some((name, value)) = part.split_once(sign)
            && credential_name(name)
            && secret_value(value)
        {
            return format!("{name}{sign}{MASK}");
        }
    }
    part.to_string()
}

/// The length of the secret's reference `text` starts with (`{{secret:NAME}}`), or none: a
/// reference is never read as a credential's name and value.
fn reference_len(text: &str) -> Option<usize> {
    let name = text.strip_prefix("{{secret:")?;
    let (name, _) = name.split_once("}}")?;
    crate::secrets::valid_name(name).then(|| "{{secret:".len() + name.len() + "}}".len())
}

/// Whether `word` says the next word is a credential: a credential's name ending in `:` or `=`
/// (or followed by one), or `Bearer`.
fn names_next(word: &str) -> bool {
    let bare = word.trim_end_matches([':', '=']);
    word.eq_ignore_ascii_case("bearer")
        || word.eq_ignore_ascii_case("token")
        || (credential_name(bare) && bare.len() < word.len())
}

/// `text` with every credential shape in it masked.
pub fn mask(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    // The last two words: a name, then `:` alone, then its value.
    let (mut before, mut last) = (String::new(), String::new());
    let flush = |word: &mut String, out: &mut String, before: &mut String, last: &mut String| {
        if word.is_empty() {
            return;
        }
        let after_name =
            names_next(last) || ((last == ":" || last == "=") && credential_name(before));
        // A sentence's full stop is not the key's.
        let bare = word.trim_end_matches(['.', '!', '?']);
        match word.get(bare.len()..) {
            Some(stop) if !stop.is_empty() && provider_key(bare) => {
                out.push_str(MASK);
                out.push_str(stop);
            }
            _ => out.push_str(&masked_word(word, after_name)),
        }
        *before = std::mem::take(last);
        *last = std::mem::take(word);
    };
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some((reference, after)) = reference_len(rest).and_then(|n| rest.split_at_checked(n))
        {
            flush(&mut word, &mut out, &mut before, &mut last);
            out.push_str(reference);
            (before, last) = (String::new(), String::new());
            rest = after;
            continue;
        }
        if boundary(c) {
            flush(&mut word, &mut out, &mut before, &mut last);
            out.push(c);
        } else {
            word.push(c);
        }
        rest = rest.get(c.len_utf8()..).unwrap_or_default();
    }
    flush(&mut word, &mut out, &mut before, &mut last);
    out
}

#[cfg(test)]
#[path = "shapes_tests.rs"]
mod tests;
