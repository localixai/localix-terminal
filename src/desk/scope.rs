//! The desk's projects (projects.rs): every call resolved to one before anything is done, the
//! person's own tabs started in the one the window shows, and what the window lists of them.

use std::path::{Path, PathBuf};

use super::Desk;
use crate::feed::{Entity, Op};
use crate::projects::{self, Project, ProjectId};

/// How many projects the switcher lists, the ones used last.
const PLACES: usize = 30;

/// Who acts for the person in a project: its own agent's key, the project, its folder, and the
/// environment its agent last called with.
pub(super) struct Person {
    pub(super) owner: String,
    pub(super) project: Project,
    pub(super) cwd: PathBuf,
    pub(super) vars: Option<Vec<(String, String)>>,
}

impl Desk {
    /// The project at `path` (absolute, made canonical), found or made, and not marked as used: only
    /// the person going to it is (what the window shows).
    ///
    /// # Errors
    /// No such folder, or the store could not keep it.
    pub fn project(&self, path: &Path) -> Result<Project, String> {
        let path = projects::canonical(path)?;
        if let Some(known) = crate::lock(&self.projects).get(&path) {
            return Ok(known.clone());
        }
        let now = crate::board::now();
        let project = (self.feed.store()).write(|tx| projects::find_or_make(tx, &path, now))?;
        crate::lock(&self.projects).insert(path, project.clone());
        // Known to this run from now: the switcher lists it, and its history menu is its own.
        self.feed.record(&Entity::Projects, Op::Upserted);
        let ledger = Entity::Ledger {
            project: project.id,
        };
        self.feed.record(&ledger, Op::Upserted);
        Ok(project)
    }

    /// The projects this run knows.
    pub(super) fn known_projects(&self) -> Vec<ProjectId> {
        (crate::lock(&self.projects).values())
            .map(|project| project.id)
            .collect()
    }

    /// What each project the desk knows is called, by its key in the window.
    pub(super) fn project_names(&self) -> std::collections::HashMap<String, String> {
        let known: Vec<Project> = crate::lock(&self.projects).values().cloned().collect();
        (projects::labels(&known).into_iter())
            .map(|(id, name)| (projects::key(id), name))
            .collect()
    }

    /// The window told the projects the desk knows, newest first, each called by its folder's
    /// name (told apart where two share one). Only the window's consumer of the feed calls it.
    pub(super) fn tell_places(&self) {
        let Some(browser) = self.browser.as_ref() else {
            return;
        };
        // The ones used last (kept across runs), the one used last first, and any other this
        // run knows after them.
        let mut known =
            (self.feed.store().read(|db| projects::recent(db, PLACES))).unwrap_or_default();
        let mut more: Vec<Project> = (crate::lock(&self.projects).values())
            .filter(|project| !known.iter().any(|recent| recent.id == project.id))
            .cloned()
            .collect();
        more.sort_by_key(|project| std::cmp::Reverse(project.id));
        known.extend(more);
        let labels = projects::labels(&known);
        let places = (known.into_iter())
            .map(|project| crate::browser::Place {
                key: projects::key(project.id),
                name: labels.get(&project.id).cloned().unwrap_or_default(),
                folder: project.path,
            })
            .collect();
        browser.places(places);
    }

    /// Project `id`, while it is kept.
    pub(super) fn project_by_id(&self, id: ProjectId) -> Option<Project> {
        let cached = crate::lock(&self.projects)
            .values()
            .find(|known| known.id == id)
            .cloned();
        cached.or_else(|| {
            (self.feed.store().read(|db| projects::get(db, id)))
                .ok()
                .flatten()
        })
    }

    /// The person picked `folder` to open: its project, found or made, is the one the window
    /// shows.
    pub(super) fn open_folder(&self, folder: &Path) {
        match (self.project(folder), self.browser.as_ref()) {
            (Ok(project), Some(browser)) => browser.go(&projects::key(project.id)),
            (Ok(_), None) => {}
            (Err(why), _) => log::warn!("{} did not open as a project: {why}", folder.display()),
        }
    }

    /// The window shows the project it keys `group` now, in window `window` (the one in use as
    /// it told it): it is the one used last, first in the switcher.
    pub(super) fn showing(&self, group: &str, window: Option<crate::layout::WindowId>) {
        let Some(id) = projects::of_key(group) else {
            return;
        };
        let now = crate::board::now();
        // The project used last, and the window's workspace of it the one it shows.
        let shown = self.feed.write(|write| {
            projects::opened(write.tx(), id, now)?;
            write.record(&Entity::Projects, Op::Upserted)?;
            Self::show_project(write, (id, window), now)
        });
        if let Err(why) = shown {
            log::warn!("the project shown was not kept as the last: {why}");
        }
    }

    /// The project used last: the window opens on it, as the person left it.
    pub(super) fn last_project(&self) -> Result<Project, String> {
        let recent = (self.feed.store()).read(|db| projects::recent(db, 1))?;
        match recent.into_iter().next() {
            Some(project) => self.project(&project.path),
            None => self.home_project(),
        }
    }

    /// Home: the project of the home folder, where what has no project goes.
    pub(super) fn home_project(&self) -> Result<Project, String> {
        self.project(&projects::home())
    }

    /// The `comment/added` notification's params for `said`, `owner`'s note: whose (its
    /// project's folder and its agent's name) and what.
    pub fn note(&self, owner: &str, said: &super::Said) -> serde_json::Value {
        let registry = crate::lock(&self.registry);
        let (project, agent) = registry
            .who(owner)
            .map_or((None, None), |(project, agent)| {
                (
                    Some(project.path.display().to_string()),
                    agent.map(str::to_string),
                )
            });
        serde_json::json!({
            "project": project,
            "agent": agent,
            "id": said.id,
            "tab": said.tab,
            "text": said.text,
        })
    }

    /// Who acts for the person in the project the window keys `group` ("": Home).
    pub(super) fn person(&self, group: &str) -> Option<Person> {
        let project = match projects::of_key(group) {
            Some(id) => self.project_by_id(id)?,
            None => self.home_project().ok()?,
        };
        let vars = crate::lock(&self.registry).vars_of(project.id);
        Some(Person {
            owner: projects::owner(&project, None),
            cwd: project.path.clone(),
            project,
            vars,
        })
    }
}
