//! Values no page can guess: the keys and the call numbers the probe and the native side share,
//! taken from the crate's one random source (`crate::random_bytes`, the kernel's generator), and
//! the seal of the run's token every message of the probe's carries.

/// A random number to name one call to a page: never 0, which names a call nobody waits for.
pub(super) fn call_id() -> u64 {
    loop {
        let id = u64::from_le_bytes(crate::random_bytes::<8>());
        if id != 0 {
            return id;
        }
    }
}

/// 32 hex characters (128 random bits): a key, or the word one call's answer must carry.
pub(super) fn word() -> String {
    crate::token()
}

/// What the probe sent: the run's token, a line break, then the message as JSON. The token is
/// plain text in front of the JSON, never a member of it, so no `toJSON` of the page's can read
/// it from the object that is stringified. None for anything else, or for JSON that does not parse.
pub(super) fn unsealed(token: &str, body: &str) -> Option<serde_json::Value> {
    let rest = body.strip_prefix(token)?.strip_prefix('\n')?;
    let parsed = serde_json::from_str(rest);
    if let Err(error) = &parsed {
        super::debug(|| format!("a page message did not parse: {error}"));
    }
    parsed.ok()
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
