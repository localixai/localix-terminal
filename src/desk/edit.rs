//! Files edited in tabs: each an editor's page told the file as the disk holds it, the disk
//! heard for as long as the tab is open, and what the person left unsaved kept with the file's
//! path, past the tab and the app. What a tab knows of the disk is edit_disk.rs.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Weak};

pub(super) use super::edit_disk::Kept;
use super::edit_disk::{Instead, NotOpened, instead, not_opened, shown, watched, within};
use super::ledger::Facts;
use super::{Call, Desk};
use crate::browser::{Pane, Panel};
use crate::editor::{self, Did, Saved};
use crate::files::drafts;
use crate::files::text::{self, Unread};
use crate::projects::ProjectId;

/// How much a file's tab is handed at once before the rest is dropped: what the person does
/// is one message a pause in typing, the disk's news one a change.
const WORK_WAITING: usize = 64;

/// What a file's worker thread is handed, in the order it came.
pub(super) enum Work {
    /// The person did this in the page.
    Posted(String),
    /// The page loaded, the first time or again: it is told the file whole.
    Loaded,
    /// Something changed in the file's folder.
    Disk,
}

impl Desk {
    /// The file at `asked` (from the caller's directory) in an editor's tab, at `line` when one
    /// is named: its tab's key. Only a file of the caller's project, as it really is: no `..`
    /// or link leads out of it. A file has one tab in a project, whoever asks: asked for again
    /// it is shown.
    pub(super) fn open_file(
        &self,
        call: &Call,
        asked: &Path,
        line: Option<u32>,
    ) -> Result<String, NotOpened> {
        let path = (call.cwd.join(asked).canonicalize())
            .map_err(|why| not_opened(asked, &Unread::from(why)))?;
        within(&path, &call.project.path)?;
        let aside = self.opens_in(call).is_some();
        let mut edits = crate::lock(&self.edits);
        let open = (edits.iter_mut())
            .find(|(_, kept)| kept.path == path && kept.project == call.project.id);
        if let Some((key, kept)) = open {
            let key = key.clone();
            match (line, &kept.panel) {
                (Some(line), Some(panel)) => drop(panel.paint(&editor::line(line))),
                _ => kept.line = line.or(kept.line),
            }
            drop(edits);
            if let Err(why) = self.reveal(call, &key) {
                log::debug!("file {key} is open, and not shown: {why}");
            }
            return Ok(key);
        }
        let held = text::read(&path).map_err(|why| not_opened(asked, &why))?;
        let n = self.next_file.fetch_add(1, Ordering::SeqCst);
        let key = format!("f{n}");
        let mut kept = Kept {
            shown: shown(&path, &call.project.path),
            project: call.project.id,
            shape: held.shape,
            held: Some(held),
            unsaved: false,
            line,
            panel: None,
            watch: None,
            path,
        };
        kept.unsaved = (self.feed.store())
            .read(|db| drafts::draft(db, &kept.path))
            .is_ok_and(|draft| draft.is_some());
        let work = self.file_worker(&key);
        kept.watch = work
            .as_ref()
            .and_then(|work| watched(&kept.path, work.clone()));
        kept.panel = work.and_then(|work| self.file_panel(n, (&key, aside), &kept, work));
        let (name, at) = (kept.name(), kept.path.clone());
        let detail = kept.shown.clone();
        edits.insert(key.clone(), kept);
        drop(edits);
        let facts = Facts {
            title: &name,
            cwd: Some(&at),
            ..Facts::default()
        };
        self.own(&key, call, crate::ledger::Kind::File, facts);
        if let Some(browser) = &self.browser {
            browser.detail(&key, Some(&detail));
        }
        Ok(key)
    }

    /// The file at `path` opened for the person, who picked it in the project's tree: in an
    /// editor's tab; what an editor does not take, as a page where a page shows its kind (a
    /// picture), else by the app they have for it. It joins the session they look at
    /// (beside.rs), where their call names no pane.
    pub(super) fn person_file(&self, call: &Call, path: &Path) -> Result<(), String> {
        let inside = (path.canonicalize()).map_err(|why| format!("{}: {why}", path.display()))?;
        let not = match self.open_file(call, &inside, None) {
            Ok(_) => return Ok(()),
            Err(not) => not,
        };
        match (instead(&inside, &not), &self.browser) {
            (Some(Instead::Page), _) => {
                let page = serde_json::json!({"kind": "browser", "url": inside});
                (self.open(call, &page))
                    .map(drop)
                    .map_err(|failure| failure.message().to_string())
            }
            (Some(Instead::Outside), Some(browser)) => {
                browser.outside(&inside);
                Ok(())
            }
            _ => Err(not.why),
        }
    }

    /// The thread that works file `key`'s tab: what the person does, the page's loads and the
    /// disk's news in the order they came. It ends with the tab, which holds every way in.
    fn file_worker(&self, key: &str) -> Option<SyncSender<Work>> {
        let (to_worker, work) = std::sync::mpsc::sync_channel::<Work>(WORK_WAITING);
        let (me, key) = (Weak::clone(&self.me), key.to_string());
        let worker = std::thread::Builder::new()
            .name("desk-file".into())
            .spawn(move || {
                for work in work {
                    let Some(desk) = me.upgrade() else { return };
                    desk.file_work(&key, work);
                }
            });
        worker
            .inspect_err(|why| log::warn!("a file's tab follows nothing (no thread: {why})"))
            .ok()
            .map(|_| to_worker)
    }

    /// The editor's panel for `kept`: what the person does in it and its loads are its
    /// worker's. `aside`, its tab opens beside the tab in sight and does not take the window.
    fn file_panel(
        &self,
        n: u32,
        (key, aside): (&str, bool),
        kept: &Kept,
        work: SyncSender<Work>,
    ) -> Option<Arc<Panel>> {
        let browser = self.browser.as_ref()?;
        let id = crate::browser::FILE.checked_add(n)?;
        let name = kept.name();
        browser.group(key, &crate::projects::key(kept.project));
        if aside {
            browser.aside(key);
        }
        let page = editor::page(&name, editor::language(&name));
        let panel = Arc::new(browser.panel(id, &name, page));
        let loaded = work.clone();
        panel.keyboard(Arc::new(move |posted: &str| {
            if work.try_send(Work::Posted(posted.to_string())).is_err() {
                log::warn!("what the person did in a file's tab was dropped: too much waits");
            }
        }));
        panel.sizing(Arc::new(move |pane, _, _| {
            if pane == Pane::Loaded && loaded.try_send(Work::Loaded).is_err() {
                log::warn!("a file was not told to its page as it loaded: too much waits");
            }
        }));
        Some(panel)
    }

    /// `work` done on file `key`'s tab: the frames its page is told, painted where a window
    /// shows it.
    pub(super) fn file_work(&self, key: &str, work: Work) -> Vec<String> {
        let mut edits = crate::lock(&self.edits);
        let Some(kept) = edits.get_mut(key) else {
            return vec![];
        };
        let frames = match work {
            Work::Disk => kept.disk(),
            Work::Loaded => self.file_told(kept),
            Work::Posted(posted) => match editor::did(&posted) {
                Some(Did::Save(mine)) => {
                    let frames = kept.save(mine);
                    if frames
                        .last()
                        .is_some_and(|last| *last == editor::saved(Saved::Kept))
                    {
                        self.file_draft(kept, None);
                    }
                    frames
                }
                Some(Did::Draft(mine)) => {
                    self.file_draft(kept, Some(&mine));
                    vec![]
                }
                Some(Did::Clean) => {
                    self.file_draft(kept, None);
                    vec![]
                }
                None => vec![],
            },
        };
        if let Some(panel) = &kept.panel {
            for frame in &frames {
                panel.paint(frame);
            }
        }
        frames
    }

    /// The frames that open `kept` in its page: the file as the disk holds it now, with what
    /// the person left unsaved in it.
    fn file_told(&self, kept: &mut Kept) -> Vec<String> {
        // Read again: the page shows the disk as it is, whatever was missed while it loaded.
        kept.disk();
        let draft = (self.feed.store().read(|db| drafts::draft(db, &kept.path)))
            .inspect_err(|why| log::warn!("a file's unsaved text was not read: {why}"))
            .ok()
            .flatten();
        let name = kept.name();
        let opened = editor::Opened {
            name: &name,
            text: kept.held.as_ref().map_or("", |held| held.text.as_str()),
            draft: draft.as_ref().map(|draft| draft.text.as_str()),
            moved: (draft.as_ref()).is_some_and(|draft| {
                (kept.held.as_ref()).is_none_or(|held| draft.moved(&held.stamp))
            }),
            line: kept.line.take(),
        };
        let mut frames = vec![editor::opened(&opened)];
        if kept.held.is_none() {
            frames.push(editor::gone());
        }
        frames
    }

    /// What the person left unsaved in `kept` is `mine` now; none: nothing is.
    fn file_draft(&self, kept: &mut Kept, mine: Option<&str>) {
        if mine.is_none() && !kept.unsaved {
            return;
        }
        let written = self.feed.write(|write| match mine {
            Some(mine) => {
                let base = kept.held.as_ref().map(|held| &held.stamp);
                drafts::keep(write.tx(), &kept.path, mine, base)
            }
            None => drafts::forget(write.tx(), &kept.path),
        });
        match written {
            Ok(()) => kept.unsaved = mine.is_some(),
            Err(why) => log::warn!("a file's unsaved text was not kept: {why}"),
        }
    }

    /// Whether tab `key` is a file of project `project`.
    pub(super) fn file_of(&self, project: ProjectId, key: &str) -> bool {
        (crate::lock(&self.edits).get(key)).is_some_and(|kept| kept.project == project)
    }

    /// The file in tab `key` as the person sees it: what they left unsaved, else the disk's.
    pub(super) fn file_text(&self, key: &str) -> Result<String, String> {
        let edits = crate::lock(&self.edits);
        let kept = (edits.get(key)).ok_or_else(|| format!("there is no file {key}"))?;
        let draft = match kept.unsaved {
            true => self.feed.store().read(|db| drafts::draft(db, &kept.path))?,
            false => None,
        };
        let disk = || kept.held.as_ref().map(|held| held.text.clone());
        Ok(draft
            .map(|draft| draft.text)
            .or_else(disk)
            .unwrap_or_default())
    }

    /// Whether the file in tab `key` went from the disk.
    #[cfg(test)]
    pub(super) fn file_gone(&self, key: &str) -> bool {
        (crate::lock(&self.edits).get(key)).is_some_and(|kept| kept.held.is_none())
    }

    /// Tab `key`'s line of `list`: the file's path, and whether text is unsaved in it.
    pub(super) fn file_summary(&self, key: &str) -> String {
        let edits = crate::lock(&self.edits);
        let Some(kept) = edits.get(key) else {
            return String::new();
        };
        let unsaved = if kept.unsaved { " (unsaved)" } else { "" };
        format!("{}{unsaved}", kept.shown)
    }

    /// File `key`'s tab is gone: its page, its worker and its watch with it. What was unsaved
    /// stays with the file.
    pub(super) fn forget_file(&self, key: &str) -> bool {
        let kept = crate::lock(&self.edits).remove(key);
        // Dropped past the lock: a watch ends its thread as it goes.
        kept.is_some()
    }

    /// Close file `key`'s tab, and say what becomes of text left unsaved in it.
    pub(super) fn close_file(&self, key: &str) -> Result<String, String> {
        let kept = crate::lock(&self.edits).remove(key);
        // Dropped past the lock, as `forget_file` does.
        let kept = kept.ok_or_else(|| format!("there is no file {key}"))?;
        Ok(match kept.unsaved {
            true => format!("Closed {key}; what was unsaved in it is kept with the file."),
            false => format!("Closed {key}."),
        })
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
