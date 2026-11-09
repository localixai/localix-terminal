//! Values no page can guess: the keys and the call numbers the probe and the native side share,
//! taken from the crate's one random source (`crate::random_bytes`, the kernel's generator).

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

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
