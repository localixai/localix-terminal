//! A file with a tab renamed or moved by the person from the tree: the tab stays the file's.
//! Its path, the folder it hears, its row in the ledger and the line under its name follow,
//! and its page is told the file again under the name it has now.

use std::path::{Path, PathBuf};

use super::Desk;
use super::edit::Work;
use super::edit_disk::{shown, watched};
use crate::feed::{Entity, Op};
use crate::ledger;
use crate::projects::ProjectId;

impl Desk {
    /// The file or folder at `from` is at `to` now: each tab of `project` on that file, or on
    /// a file under that folder, is on it where it is.
    pub(super) fn files_moved(&self, project: ProjectId, from: &Path, to: &Path) {
        let Some(root) = self.project_by_id(project).map(|known| known.path) else {
            return;
        };
        let mut edits = crate::lock(&self.edits);
        let mut moved: Vec<(String, PathBuf, String)> = (edits.iter_mut())
            .filter(|(_, kept)| kept.project == project)
            .filter_map(|(key, kept)| {
                let rest = kept.path.strip_prefix(from).ok()?;
                kept.path = match rest.as_os_str().is_empty() {
                    true => to.to_path_buf(),
                    false => to.join(rest),
                };
                kept.shown = shown(&kept.path, &root);
                // The folder it is in now is the one heard; the page is told the file again.
                kept.watch = (kept.work.clone()).and_then(|work| watched(&kept.path, work));
                if let Some(work) = &kept.work {
                    let _ = work.try_send(Work::Loaded);
                }
                Some((key.clone(), kept.path.clone(), kept.shown.clone()))
            })
            .collect();
        drop(edits);
        moved.sort();
        for (key, path, shown) in moved {
            let name = (path.file_name())
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            self.file_placed(&key, &name, &path);
            if let Some(browser) = &self.browser {
                browser.detail(&key, Some(&shown));
            }
        }
    }

    /// File `key`'s tab is on the file at `path` now, named `title`: its row says so, so it
    /// opens again from there, and its project's past is news.
    fn file_placed(&self, key: &str, title: &str, path: &Path) {
        let Some((row, project)) = crate::lock(&self.ledger).get(key).copied() else {
            return;
        };
        let placed = self.feed.write(|write| {
            ledger::moved(write.tx(), row, title, path)?;
            write.record(&Entity::Ledger { project }, Op::Upserted)
        });
        if let Err(why) = placed {
            log::warn!("tab {key}'s new place is not in the ledger: {why}");
        }
    }
}
