//! Screenshots saved as files an agent can read; the newest few are kept. The app sends a
//! picture as bytes, and the CLI, whose caller reads files, saves it here.

use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Pictures kept in the directory.
const KEPT: usize = 30;

/// `jpeg` as a file in `dir`; its path.
pub fn save(dir: &Path, jpeg: &[u8]) -> Result<PathBuf, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    // The person's own: a picture is of their page.
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    // The process too: the app and each CLI process count from 0 on their own.
    let name = format!(
        "shot-{stamp}-{}-{}.jpg",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let path = dir.join(name);
    // Never over a file that is there: it may be the picture another agent was just handed.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(jpeg)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    prune(dir);
    Ok(path)
}

/// Remove all but the newest `KEPT` pictures.
pub fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("shot-"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    files.sort_unstable();
    let old = files.len().saturating_sub(KEPT);
    for (_, path) in files.into_iter().take(old) {
        let _ = std::fs::remove_file(path);
    }
}
