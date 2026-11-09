//! Values no page can guess: the keys and the call numbers the probe and the native side share,
//! read from the system's random source (the kernel's generator, never the clock).

use std::fmt::Write as _;

/// `buf` filled from the system's random source.
#[cfg(target_os = "macos")]
fn fill(buf: &mut [u8]) {
    // SAFETY: the pointer and length name `buf`, which is writable for the whole call;
    // arc4random_buf never fails and writes exactly that many bytes.
    unsafe { libc::arc4random_buf(buf.as_mut_ptr().cast(), buf.len()) }
}

/// `buf` filled from the system's random source: getrandom may fill part of it, or be
/// interrupted by a signal, so it is asked again for what is left.
#[cfg(target_os = "linux")]
fn fill(buf: &mut [u8]) {
    let mut done = 0;
    while done < buf.len() {
        let left = &mut buf[done..];
        // SAFETY: the pointer and length name `left`, writable for the call.
        let read = unsafe { libc::getrandom(left.as_mut_ptr().cast(), left.len(), 0) };
        match usize::try_from(read) {
            Ok(read) => done += read,
            Err(_) if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted => {
            }
            // The generator refused (no such call on a very old kernel): the hasher's keys, which
            // std seeds from the system's random source too.
            Err(_) => return hashed(left),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn fill(buf: &mut [u8]) {
    hashed(buf);
}

/// `buf` filled from std's randomly keyed hasher, where the system's generator cannot be asked.
#[cfg(not(target_os = "macos"))]
fn hashed(buf: &mut [u8]) {
    use std::hash::{BuildHasher, Hasher};
    for chunk in buf.chunks_mut(8) {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_usize(chunk.as_ptr() as usize);
        let bytes = hasher.finish().to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
}

/// A random number to name one call to a page: never 0, which names a call nobody waits for.
pub(super) fn call_id() -> u64 {
    loop {
        let mut bytes = [0; 8];
        fill(&mut bytes);
        let id = u64::from_le_bytes(bytes);
        if id != 0 {
            return id;
        }
    }
}

/// 32 hex characters (128 random bits): a key, or the word one call's answer must carry.
pub(super) fn word() -> String {
    let mut bytes = [0; 16];
    fill(&mut bytes);
    bytes
        .iter()
        .fold(String::with_capacity(32), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
