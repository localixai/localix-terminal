//! A file edited in a tab, against the disk: what the page was last told the disk holds, the
//! news of what it holds now, a save that writes only over what the page was told, and the
//! file's folder heard while its tab is open.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use super::edit::Work;
use crate::browser::Panel;
use crate::editor::{self, Saved};
use crate::files::Watch;
use crate::files::text::{self, Shape, Text, Unread};
use crate::projects::ProjectId;

/// Why a file got no tab.
#[derive(Debug)]
pub(super) struct NotOpened {
    pub(super) why: String,
    /// It is there, and not text an editor takes: a page's to show (a picture, a film), or
    /// to say in a line that nothing shows it.
    pub(super) page: bool,
}

/// A file edited in a tab.
pub(super) struct Kept {
    /// The file's real path, and its project.
    pub(super) path: PathBuf,
    pub(super) project: ProjectId,
    /// The path as the tab says it: from the project's folder, where it is under it.
    pub(super) shown: String,
    /// What the page was last told the disk holds; none: the file is gone.
    pub(super) held: Option<Text>,
    /// How the disk last wrote the file's text: a file that went is written so again.
    pub(super) shape: Shape,
    pub(super) unsaved: bool,
    /// The line to show as the page loads, once.
    pub(super) line: Option<u32>,
    pub(super) panel: Option<Arc<Panel>>,
    /// The file's folder, heard while the tab is open.
    pub(super) watch: Option<Watch>,
    /// Where its worker is handed work: a file that moved is heard, and told, from there.
    pub(super) work: Option<SyncSender<Work>>,
}

impl Kept {
    pub(super) fn name(&self) -> String {
        (self.path.file_name()).map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }

    /// The file's text as the page was last told the disk holds it; empty once the file is gone.
    pub(super) fn text(&self) -> &str {
        self.held.as_ref().map_or("", |held| held.text.as_str())
    }

    /// The frames that tell the page what the disk holds now, when it is not what the page
    /// was told. What an editor does not take (the file made a picture) is not told: the page
    /// keeps its text, and a save says why it is not written.
    pub(super) fn disk(&mut self) -> Vec<String> {
        let now = match &self.held {
            Some(held) => match text::news(&self.path, &held.stamp) {
                Some(now) => now,
                None => return vec![],
            },
            None => text::read(&self.path),
        };
        match now {
            Ok(now) => {
                let frame = editor::disk(&now.text);
                self.shape = now.shape;
                self.held = Some(now);
                vec![frame]
            }
            Err(Unread::Gone) if self.held.take().is_some() => vec![editor::gone()],
            Err(_) => vec![],
        }
    }

    /// `mine` written as the file, unless another hand wrote the file since the page was
    /// told: then nothing is written, and the page is told what the disk holds. A file that
    /// went is made again.
    pub(super) fn save(&mut self, mine: String) -> Vec<String> {
        let now = match &self.held {
            Some(held) => text::news(&self.path, &held.stamp),
            None => Some(text::read(&self.path)),
        };
        match now {
            Some(Ok(now)) => {
                let frame = editor::disk(&now.text);
                self.shape = now.shape;
                self.held = Some(now);
                return vec![frame, editor::saved(Saved::Conflict)];
            }
            Some(Err(Unread::Gone)) | None => {}
            Some(Err(_)) => {
                let why = "the file on disk is no longer text an editor takes";
                return vec![editor::saved(Saved::Failed(why))];
            }
        }
        match text::save(&self.path, &mine, self.shape) {
            Ok(stamp) => {
                self.held = Some(Text {
                    text: mine,
                    shape: self.shape,
                    stamp,
                });
                vec![editor::saved(Saved::Kept)]
            }
            Err(why) => vec![editor::saved(Saved::Failed(&why.to_string()))],
        }
    }
}

/// `path` said as a tab says it: from `root` where it is under it.
pub(super) fn shown(path: &Path, root: &Path) -> String {
    let under = root.canonicalize().ok();
    let short = under.and_then(|root| path.strip_prefix(root).ok().map(Path::to_path_buf));
    short.as_deref().unwrap_or(path).display().to_string()
}

/// The file at `path` (as it really is) held to its project at `root`: a tab is of one project,
/// and what an agent reads through it is that project's.
pub(super) fn within(path: &Path, root: &Path) -> Result<(), NotOpened> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    match path.starts_with(&root) {
        true => Ok(()),
        false => Err(NotOpened {
            why: format!(
                "{} is not inside {}: only the project's files open in an editor",
                path.display(),
                root.display()
            ),
            page: false,
        }),
    }
}

/// Why `asked` is not read as text, in words for whoever asked for its tab.
pub(super) fn not_opened(asked: &Path, why: &Unread) -> NotOpened {
    let said = asked.display();
    let (why, page) = match why {
        Unread::Gone => (format!("there is no file {said}"), false),
        Unread::Folder => (
            format!("{said} is a folder: an editor's tab holds one file"),
            false,
        ),
        Unread::Large(len) => (
            format!(
                "{said} is {} MB: an editor takes text up to {} MB",
                len.div_ceil(1024 * 1024),
                text::LARGEST / (1024 * 1024)
            ),
            true,
        ),
        Unread::Binary => (
            format!("{said} is not text (UTF-8): an editor does not take it"),
            true,
        ),
        Unread::Failed(why) => (format!("{said} could not be read: {why}"), false),
    };
    NotOpened { why, page }
}

/// The folder of the file at `path` heard: each change in it is `work`'s to look at.
pub(super) fn watched(path: &Path, work: SyncSender<Work>) -> Option<Watch> {
    let folder = path.parent()?;
    let watch = Watch::new(folder, move |_| {
        // Full: a look at the disk waits already, and sees this change too.
        let _ = work.try_send(Work::Disk);
    });
    let watch = watch
        .inspect_err(|why| log::warn!("files: {} is not watched: {why}", folder.display()))
        .ok()?;
    watch.listen(&BTreeSet::from([folder.to_path_buf()]));
    Some(watch)
}
