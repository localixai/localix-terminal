//! What the person does to a project's files from its tree (`files::ops`): an entry made,
//! renamed, moved into another folder, moved to the Trash, or given to a terminal. Done on the
//! writer thread, then told at once: the folders it changed are read again without waiting for
//! the system to say so, the entry is picked where it is now, and the folders left open, the
//! tabs and what is unsaved follow it. What is refused changes nothing and is said in the tree.

use std::path::Path;
use std::sync::Arc;

use super::Desk;
use crate::browser::{Asked, EntryDid};
use crate::files::drafts;
use crate::files::ops::{self, Refused};
use crate::files::{self};
use crate::projects::{self, ProjectId};

/// What the person did to an entry, kept past the window's call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Op {
    Made {
        dir: String,
        name: String,
        folder: bool,
    },
    Named {
        path: String,
        name: String,
    },
    Moved {
        path: String,
        into: String,
    },
    Trashed(String),
    Given {
        path: String,
        terminal: String,
    },
}

impl From<EntryDid<'_>> for Op {
    fn from(did: EntryDid<'_>) -> Self {
        match did {
            EntryDid::Made { dir, name, folder } => Self::Made {
                dir: dir.into(),
                name: name.into(),
                folder,
            },
            EntryDid::Named { path, name } => Self::Named {
                path: path.into(),
                name: name.into(),
            },
            EntryDid::Moved { path, into } => Self::Moved {
                path: path.into(),
                into: into.into(),
            },
            EntryDid::Trashed(path) => Self::Trashed(path.into()),
            EntryDid::Given { path, terminal } => Self::Given {
                path: path.into(),
                terminal: terminal.into(),
            },
        }
    }
}

impl Op {
    /// The entry it is about, where the tree says what was refused: the folder for one made.
    fn about(&self) -> &str {
        match self {
            Self::Made { dir, .. } => dir,
            Self::Named { path, .. }
            | Self::Moved { path, .. }
            | Self::Given { path, .. }
            | Self::Trashed(path) => path,
        }
    }
}

/// The folder the entry at `path` is in.
fn above(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(above, _)| above)
}

/// `path` as it is typed into a terminal: one name to a shell, whatever it holds, and a space
/// after it, as a file dropped on a terminal is.
pub(super) fn typed(path: &Path) -> String {
    let mut typed = String::new();
    for letter in path.to_string_lossy().chars() {
        let plain = letter.is_alphanumeric() || "_@%+=:,./-".contains(letter);
        if !plain {
            typed.push('\\');
        }
        typed.push(letter);
    }
    typed.push(' ');
    typed
}

/// The entry at `path` moved to the system's Trash. Under test it only goes: no test leaves
/// anything in the Trash of whoever runs it.
fn trash(path: &Path) -> Result<(), Refused> {
    if cfg!(test) {
        let gone = match path.symlink_metadata()? {
            entry if entry.is_dir() => std::fs::remove_dir_all(path),
            _ => std::fs::remove_file(path),
        };
        return gone.map_err(Refused::from);
    }
    path.symlink_metadata()?;
    crate::browser::trash(path).map_err(Refused::Failed)
}

impl Desk {
    /// `op` done to `project`'s files, and what is refused said in its tree, under the entry.
    pub(super) fn tree_op(self: &Arc<Self>, project: ProjectId, op: &Op) {
        let done = self.tree_does(project, op);
        let key = projects::key(project);
        match (done, op, &self.browser) {
            // A file just made is one to write in: it opens as one picked in the tree does.
            (
                Ok(()),
                Op::Made {
                    dir,
                    name,
                    folder: false,
                },
                _,
            ) => {
                let made = (self.tree_root(project))
                    .and_then(|root| files::within(&root, dir).map(|dir| dir.join(name)));
                if let Some(made) = made {
                    self.person_asked(&key, Asked::File(made));
                }
            }
            (Err(refused), _, Some(browser)) => {
                browser.tree_refused(&key, op.about(), &refused.said());
            }
            (Ok(()) | Err(_), ..) => {}
        }
    }

    /// `op` done to `project`'s files, and its tree set to tell of it. Nothing of a tree no
    /// window shows is touched.
    pub(super) fn tree_does(&self, project: ProjectId, op: &Op) -> Result<(), Refused> {
        let root = self.tree_root(project).ok_or(Refused::Gone)?;
        match op {
            Op::Made { dir, name, folder } => {
                let made = ops::make(&root, dir, name, *folder)?;
                self.tree_kept(|tx| match dir.is_empty() {
                    true => Ok(()),
                    false => files::open(tx, project, dir).map(drop),
                });
                self.tree_touched(project, &[dir], Some(made));
            }
            Op::Named { path, name } => {
                let now = ops::rename(&root, path, name)?;
                self.tree_followed(project, &root, path, &now);
                self.tree_touched(project, &[above(path)], Some(now));
            }
            Op::Moved { path, into } => {
                let now = ops::move_into(&root, path, into)?;
                if now != *path {
                    self.tree_kept(|tx| match into.is_empty() {
                        true => Ok(()),
                        false => files::open(tx, project, into).map(drop),
                    });
                    self.tree_followed(project, &root, path, &now);
                }
                self.tree_touched(project, &[above(path), into], Some(now));
            }
            Op::Trashed(path) => {
                let at = (files::within(&root, path).filter(|_| !path.is_empty()))
                    .ok_or(Refused::Gone)?;
                trash(&at)?;
                self.tree_kept(|tx| files::shut(tx, project, path).map(drop));
                self.tree_touched(project, &[above(path)], None);
            }
            Op::Given { path, terminal } => {
                let at = files::within(&root, path).ok_or(Refused::Gone)?;
                let id = terminal.strip_prefix('t').and_then(|id| id.parse().ok());
                let session = id.and_then(|id| self.session(id).ok());
                (session.ok_or(Refused::Gone)?)
                    .type_in(&typed(&at), &[])
                    .map_err(Refused::Failed)?;
            }
        }
        Ok(())
    }

    /// The entry at `from` of `project`'s tree is at `to` now: a folder stays as open as it
    /// was, what is unsaved stays with its file, and the tabs of the files there follow.
    fn tree_followed(&self, project: ProjectId, root: &Path, from: &str, to: &str) {
        let (was, now) = (root.join(from), root.join(to));
        self.tree_kept(|tx| {
            files::moved(tx, project, from, to)?;
            drafts::moved(tx, &was, &now)
        });
        self.files_moved(project, &was, &now);
    }

    /// `write` kept in the store; what is not is only where a folder stays open or a draft
    /// stays, and is logged.
    fn tree_kept(&self, write: impl FnOnce(&crate::store::Transaction<'_>) -> Result<(), String>) {
        if let Err(why) = self.feed.store().write(write) {
            log::warn!("files: what followed an entry was not kept: {why}");
        }
    }
}

#[cfg(test)]
#[path = "files_ops_tests.rs"]
mod tests;
