//! The trees of files in sight in the window. A project's tree is read and watched only while a
//! window shows it: its open folders are the store's (`files::kept`), each listed once and again
//! only when the system says it changed. The window is told what is new to it, never the whole
//! tree again, through the feed's one consumer for the window (`layout_drawn.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::Desk;
use super::files_ops::Op as EntryOp;
use crate::browser::{Branch, Leaf, Tree, TreeDid};
use crate::feed::{Entity, Op};
use crate::files::{self, Change, Listing, Watch};
use crate::projects::{self, ProjectId};

/// A tree in sight: what the window was last told of it, and what must be read again.
pub(super) struct Shown {
    /// The project's folder, by its real path: the system names what changed by it.
    root: PathBuf,
    /// The system's notices of its folders; none where it gave none (the tree shows, and
    /// follows only what the person does in it).
    watch: Option<Watch>,
    /// Each open folder as the window was last told it, by its path under the root.
    listed: BTreeMap<String, Listing>,
    stale: Stale,
    /// Whether a notice the window's consumer has not pulled yet is in the feed: a burst of
    /// changes is one notice.
    noticed: bool,
    /// The entry the person just made, named or moved, to be picked as the tree is next told.
    pick: Option<String>,
}

/// What of a tree is to be read again.
enum Stale {
    Dirs(BTreeSet<String>),
    All,
}

/// What the person did in a tree, kept past the window's call.
enum Did {
    Seen(bool),
    Opened(String),
    Shut(String),
    Entry(EntryOp),
}

impl Desk {
    /// What the person did in the tree of files of the project `key` names. It is done on the
    /// writer thread: the window's loop reads no folder and waits on no disk.
    pub(super) fn person_on_tree(self: &Arc<Self>, key: &str, did: TreeDid<'_>) {
        let Some(project) = projects::of_key(key) else {
            return;
        };
        let did = match did {
            TreeDid::Seen(seen) => Did::Seen(seen),
            TreeDid::Opened(path) => Did::Opened(path.to_string()),
            TreeDid::Shut(path) => Did::Shut(path.to_string()),
            TreeDid::Entry(did) => Did::Entry(did.into()),
        };
        let desk = Arc::clone(self);
        self.write_later(move || match did {
            Did::Seen(seen) => desk.tree_seen(project, seen),
            Did::Opened(path) => desk.tree_folder(project, &path, true),
            Did::Shut(path) => desk.tree_folder(project, &path, false),
            Did::Entry(op) => desk.tree_op(project, &op),
        });
    }

    /// Project `project`'s tree is in sight from now on (read whole, then watched), or in none
    /// any more (let go of: nothing of it is read or heard).
    pub(super) fn tree_seen(self: &Arc<Self>, project: ProjectId, seen: bool) {
        if !seen {
            crate::lock(&self.trees).remove(&project);
            return;
        }
        if crate::lock(&self.trees).contains_key(&project) {
            return;
        }
        let Some(known) = self.project_by_id(project) else {
            return;
        };
        let root = known.path.canonicalize().unwrap_or(known.path);
        let desk = Arc::downgrade(self);
        let watch = Watch::new(&root, move |change| {
            if let Some(desk) = desk.upgrade() {
                desk.tree_changed(project, change);
            }
        });
        let watch = watch
            .inspect_err(|why| log::warn!("files: {} is not watched: {why}", root.display()))
            .ok();
        let shown = Shown {
            root,
            watch,
            listed: BTreeMap::new(),
            stale: Stale::All,
            noticed: true,
            pick: None,
        };
        crate::lock(&self.trees).insert(project, shown);
        self.feed.record(&Entity::Files { project }, Op::Upserted);
    }

    /// The person opened (`open`) or shut folder `path` of `project`'s tree: kept so, and the
    /// window told. A path that names no folder open to them is passed over.
    pub(super) fn tree_folder(&self, project: ProjectId, path: &str, open: bool) {
        {
            let trees = crate::lock(&self.trees);
            let Some(shown) = trees.get(&project) else {
                return;
            };
            let above = path.rsplit_once('/').map_or("", |(above, _)| above);
            let reachable = !path.is_empty()
                && files::within(&shown.root, path).is_some()
                && shown.listed.contains_key(above);
            if open && !reachable {
                return;
            }
        }
        let kept = self.feed.write(|write| {
            let changed = if open {
                files::open(write.tx(), project, path)?
            } else {
                files::shut(write.tx(), project, path)?
            };
            if changed {
                write.record(&Entity::Files { project }, Op::Upserted)?;
            }
            Ok(())
        });
        if let Err(why) = kept {
            log::warn!("files: a folder's place in the tree was not kept: {why}");
        }
    }

    /// The system said `change` happened under `project`'s folder, on the watch's own thread:
    /// what to read again is marked, and one notice says so.
    fn tree_changed(&self, project: ProjectId, change: Change) {
        let notice = {
            let mut trees = crate::lock(&self.trees);
            let Some(shown) = trees.get_mut(&project) else {
                return;
            };
            match change {
                Change::All => shown.stale = Stale::All,
                Change::Dirs(dirs) => {
                    for dir in dirs {
                        let rel = dir.strip_prefix(&shown.root).ok().and_then(rel_of);
                        match (&mut shown.stale, rel) {
                            (Stale::Dirs(stale), Some(rel)) => {
                                stale.insert(rel);
                            }
                            (Stale::All, _) | (_, None) => {}
                        }
                    }
                }
            }
            !std::mem::replace(&mut shown.noticed, true)
        };
        if notice {
            self.feed.record(&Entity::Files { project }, Op::Upserted);
        }
    }

    /// The folder of `project`'s tree, while a window shows it.
    pub(super) fn tree_root(&self, project: ProjectId) -> Option<PathBuf> {
        (crate::lock(&self.trees).get(&project)).map(|shown| shown.root.clone())
    }

    /// The person's own hand changed folders `dirs` of `project`'s tree: they are read again
    /// now, not when the system gets to saying so, and `pick` is the entry picked then.
    pub(super) fn tree_touched(&self, project: ProjectId, dirs: &[&str], pick: Option<String>) {
        let notice = {
            let mut trees = crate::lock(&self.trees);
            let Some(shown) = trees.get_mut(&project) else {
                return;
            };
            if let Stale::Dirs(stale) = &mut shown.stale {
                stale.extend(dirs.iter().map(ToString::to_string));
            }
            shown.pick = pick;
            !std::mem::replace(&mut shown.noticed, true)
        };
        if notice {
            self.feed.record(&Entity::Files { project }, Op::Upserted);
        }
    }

    /// The window told what is new of `project`'s tree. Only the window's consumer of the feed
    /// calls it.
    pub(super) fn tell_files(&self, project: ProjectId) {
        let Some(browser) = self.browser.as_ref() else {
            return;
        };
        if let Some(tree) = self.tree_news(project) {
            browser.tree(&projects::key(project), tree);
        }
    }

    /// What is new of `project`'s tree since it was last asked: none while it is in no window's
    /// sight, or when nothing the window draws changed.
    pub(super) fn tree_news(&self, project: ProjectId) -> Option<Tree> {
        let (root, stale, known) = {
            let mut trees = crate::lock(&self.trees);
            let shown = trees.get_mut(&project)?;
            shown.noticed = false;
            let stale = std::mem::replace(&mut shown.stale, Stale::Dirs(BTreeSet::new()));
            let known: BTreeSet<String> = shown.listed.keys().cloned().collect();
            (shown.root.clone(), stale, known)
        };
        let kept = (self.feed.store().read(|db| files::open_dirs(db, project)))
            .inspect_err(|why| log::warn!("files: the open folders could not be read: {why}"))
            .unwrap_or_default();
        let mut open: BTreeSet<String> = kept.into_iter().collect();
        open.insert(String::new());
        // Heard before read: a change that comes while a folder is being read is told after.
        self.listen(project, &root, &open);
        let read: Vec<&String> = (open.iter())
            .filter(|path| match &stale {
                Stale::All => true,
                Stale::Dirs(dirs) => dirs.contains(*path) || !known.contains(*path),
            })
            .collect();
        let mut fresh: BTreeMap<String, Option<Listing>> = BTreeMap::new();
        for path in read {
            let listing = files::within(&root, path).and_then(|dir| files::list(&dir).ok());
            fresh.insert(path.clone(), listing);
        }
        self.tree_told(project, &root, open, fresh)
    }

    /// The folders the system tells of for `project` are its open ones.
    fn listen(&self, project: ProjectId, root: &Path, open: &BTreeSet<String>) {
        let dirs: BTreeSet<PathBuf> = (open.iter())
            .filter_map(|path| files::within(root, path))
            .collect();
        let trees = crate::lock(&self.trees);
        if let Some(watch) = trees.get(&project).and_then(|shown| shown.watch.as_ref()) {
            watch.listen(&dirs);
        }
    }

    /// `fresh` (each folder just read; none: it could not be) taken into what the window was
    /// told of `project`'s tree, and what of it is news. A folder that is gone, or no longer
    /// under an open folder that lists it, is shut for good.
    fn tree_told(
        &self,
        project: ProjectId,
        root: &Path,
        mut open: BTreeSet<String>,
        fresh: BTreeMap<String, Option<Listing>>,
    ) -> Option<Tree> {
        let mut trees = crate::lock(&self.trees);
        let shown = trees.get_mut(&project).filter(|shown| shown.root == root)?;
        let before = shown.listed.len();
        let (mut news, mut gone) = (vec![], vec![]);
        for (path, listing) in fresh {
            // The root that cannot be read shows as empty; any other folder is gone.
            let listing = match listing {
                Some(listing) => listing,
                None if path.is_empty() => Listing::default(),
                None => {
                    open.remove(&path);
                    gone.push(path);
                    continue;
                }
            };
            if shown.listed.get(&path) != Some(&listing) {
                shown.listed.insert(path.clone(), listing);
                news.push(path);
            }
        }
        // Parents come first in the order of paths, so one pass settles a whole branch.
        for path in open.clone() {
            if path.is_empty() {
                continue;
            }
            let (above, name) = path.rsplit_once('/').unwrap_or(("", &path));
            let listed = open.contains(above)
                && (shown.listed.get(above)).is_some_and(|dir| {
                    (dir.entries.iter()).any(|entry| entry.dir && entry.name == name)
                });
            if !listed {
                open.remove(&path);
                gone.push(path);
            }
        }
        shown.listed.retain(|path, _| open.contains(path));
        let root = shown.root.clone();
        let dirs: Vec<Branch> = (news.iter())
            .filter_map(|path| Some((path, shown.listed.get(path)?)))
            .map(|(path, listing)| branch(path, listing))
            .collect();
        let pick = shown.pick.take();
        let changed = !dirs.is_empty() || shown.listed.len() != before || pick.is_some();
        drop(trees);
        if !gone.is_empty() {
            let forgot = self.feed.store().write(|tx| {
                (gone.iter()).try_for_each(|path| files::shut(tx, project, path).map(drop))
            });
            if let Err(why) = forgot {
                log::warn!("files: a folder that is gone was not forgotten: {why}");
            }
        }
        changed.then(|| Tree {
            root,
            open: open.into_iter().collect(),
            dirs,
            pick,
        })
    }
}

/// A folder under the root as the tree writes it: its names joined by `/`.
fn rel_of(under: &Path) -> Option<String> {
    let names: Option<Vec<&str>> = under.iter().map(|name| name.to_str()).collect();
    Some(names?.join("/"))
}

/// A folder's listing as the window draws it.
fn branch(path: &str, listing: &Listing) -> Branch {
    Branch {
        path: path.to_string(),
        leaves: (listing.entries.iter())
            .map(|entry| Leaf {
                name: entry.name.clone(),
                dir: entry.dir,
            })
            .collect(),
        more: listing.more,
    }
}

#[cfg(test)]
#[path = "files_tests.rs"]
pub(super) mod tests;
