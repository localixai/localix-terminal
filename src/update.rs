//! The app kept up to date: a newer release found, fetched, checked and made ready while the
//! person works, then put in the app's place when they say so, and the app started again.
//!
//! `Updater` is the one owner of how far that is (`Stage`); the Event Center, Settings and the
//! menu read it and ask it, and none keeps a copy. Only an app installed from a release is
//! updated: a build of another making (from source, a package) says why not and asks nothing
//! of the network.

mod fetch;
mod install;
mod place;
mod release;
mod run;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub use place::Again;
pub use release::REPO;
pub use release::{Release, Version};

use release::From;

/// How long after the app starts the first check of a watch is, and how long between checks.
const FIRST_AFTER: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(4 * 60 * 60);

/// This app's updater: one for the process.
pub fn app() -> &'static Updater {
    static APP: OnceLock<Updater> = OnceLock::new();
    APP.get_or_init(Updater::of_this_app)
}

/// How long the host may take to say what the latest release is.
const LATEST_MOST: Duration = Duration::from_secs(30);

/// How far an update is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// Nothing newer is known of.
    Idle,
    /// The host is being asked.
    Checking,
    /// A newer release is being fetched: `got` bytes of `of`, where the release says its size.
    Fetching {
        release: Release,
        got: u64,
        of: Option<u64>,
    },
    /// It is fetched, checked and waits to be put in place.
    Ready { release: Release },
    /// The last check or fetch did not end well; the next one starts over.
    Failed { why: String },
}

/// What the updater says of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Told {
    /// This build's version.
    pub current: String,
    pub stage: Stage,
    /// When the host last answered, in seconds of the epoch.
    pub checked: Option<u64>,
    /// Why this build is not updated by the app, when it is not.
    pub unable: Option<String>,
}

/// What an updater works with: given whole, so a test stands in for the host and the app.
pub(crate) struct Given {
    pub(crate) from: From,
    pub(crate) current: String,
    /// The archive a release has for this system; none where it has no build.
    pub(crate) archive: Option<&'static str>,
    /// Where an update is made ready.
    pub(crate) dir: PathBuf,
    /// Where the app is, or why it is not one the app replaces. Asked once, at the first check.
    pub(crate) place: Box<dyn Fn() -> Result<PathBuf, String> + Send + Sync>,
    /// Whether what was unpacked is the app.
    pub(crate) trusted: fn(&Path) -> Result<(), String>,
}

struct Inner {
    given: Given,
    /// Where the app is, or why it is not updated here: kept once it is known where, and
    /// asked again with each check until then.
    place: Mutex<Option<Result<PathBuf, String>>>,
    /// The update that is ready is being put in place: once, however often it is asked.
    applying: AtomicBool,
    state: Mutex<(Stage, Option<u64>)>,
    /// A check or a fetch runs.
    busy: AtomicBool,
    changed: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

/// The app's updater.
#[derive(Clone)]
pub struct Updater(Arc<Inner>);

impl Updater {
    pub(crate) fn new(given: Given) -> Self {
        Self(Arc::new(Inner {
            given,
            place: Mutex::new(None),
            applying: AtomicBool::new(false),
            state: Mutex::new((Stage::Idle, None)),
            busy: AtomicBool::new(false),
            changed: OnceLock::new(),
        }))
    }

    /// The updater of this app, reading the project's releases. In a build for development
    /// `LLX_DESK_UPDATE_FROM` names another place that holds them; a release reads its own
    /// alone, whatever its environment says.
    pub fn of_this_app() -> Self {
        let from = std::env::var("LLX_DESK_UPDATE_FROM").ok();
        let from = from.filter(|from| cfg!(debug_assertions) && !from.is_empty());
        Self::new(Given {
            from: from.map_or_else(From::default, |base| From::at(&base)),
            current: env!("CARGO_PKG_VERSION").to_string(),
            archive: release::archive(std::env::consts::OS, std::env::consts::ARCH),
            dir: crate::paths::data().join("update"),
            place: Box::new(place::of_this_app),
            trusted: install::trusted,
        })
    }

    /// `changed` is called whenever what [`Updater::told`] answers changed. Given once.
    pub fn on_change(&self, changed: impl Fn() + Send + Sync + 'static) {
        let _ = self.0.changed.set(Box::new(changed));
    }

    pub fn told(&self) -> Told {
        let (stage, checked) = crate::lock(&self.0.state).clone();
        Told {
            current: self.0.given.current.clone(),
            stage,
            checked,
            // Not asked for here: it is known once a check ran.
            unable: (crate::lock(&self.0.place).clone()).and_then(Result::err),
        }
    }

    fn set(&self, stage: Stage) {
        {
            let mut state = crate::lock(&self.0.state);
            if state.0 == stage {
                return;
            }
            state.0 = stage;
        }
        if let Some(changed) = self.0.changed.get() {
            changed();
        }
    }

    /// Where the app is, or why it is not updated here.
    fn place(&self) -> Result<PathBuf, String> {
        if let Some(Ok(place)) = &*crate::lock(&self.0.place) {
            return Ok(place.clone());
        }
        // Asked with no lock held: it runs the system's tools.
        let given = &self.0.given;
        let place = match given.archive {
            Some(_) => (given.place)(),
            None => Err("There is no release build for this system.".into()),
        };
        *crate::lock(&self.0.place) = Some(place.clone());
        place
    }

    /// Where the app is, as it was last asked: nothing is run.
    fn placed(&self) -> Result<PathBuf, String> {
        (crate::lock(&self.0.place).clone()).unwrap_or_else(|| Err("No update is ready.".into()))
    }

    /// The host asked for the latest release, and a newer one fetched and made ready; on a
    /// thread of its own. Nothing happens while one runs, or when this build is not updated.
    pub fn check(&self) {
        let updater = self.clone();
        let spawned = std::thread::Builder::new()
            .name("update".into())
            .spawn(move || updater.check_now());
        if let Err(why) = spawned {
            log::warn!("update: no check (no thread: {why})");
        }
    }

    /// Checks from here on, on a thread of its own: soon after the app starts, then every few
    /// hours, each only while `wanted` says the person wants them made for them.
    pub fn watch(&self, wanted: impl Fn() -> bool + Send + 'static) {
        let updater = self.clone();
        let watching = std::thread::Builder::new()
            .name("update-watch".into())
            .spawn(move || {
                // Nothing is ever sent: the wait is the deadline, and ends with the process.
                let (_held, wait) = std::sync::mpsc::sync_channel::<()>(0);
                let mut after = FIRST_AFTER;
                while wait.recv_timeout(after).is_err() {
                    if wanted() {
                        updater.check_now();
                    }
                    after = EVERY;
                }
            });
        if let Err(why) = watching {
            log::warn!("update: no checks (no thread: {why})");
        }
    }

    /// As [`Updater::check`], on this thread.
    pub(crate) fn check_now(&self) {
        if self.0.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        if self.place().is_ok() {
            let before = crate::lock(&self.0.state).0.clone();
            if let Err(why) = self.checked(&before) {
                log::warn!("update: {why}");
                // What is ready stays ready: a host out of reach takes nothing back. Once a
                // newer one was begun the ready one made room for it, and nothing is.
                let ready = matches!(crate::lock(&self.0.state).0, Stage::Ready { .. });
                if !ready {
                    self.set(Stage::Failed { why });
                }
            }
        } else if let Some(changed) = self.0.changed.get() {
            // Why not is known from here on.
            changed();
        }
        self.0.busy.store(false, Ordering::SeqCst);
    }

    fn checked(&self, before: &Stage) -> Result<(), String> {
        let given = &self.0.given;
        let Some(name) = given.archive else {
            return Ok(());
        };
        let ready = match before {
            Stage::Ready { release } => Some(release.version),
            Stage::Idle | Stage::Checking | Stage::Fetching { .. } | Stage::Failed { .. } => {
                self.set(Stage::Checking);
                None
            }
        };
        let latest = fetch::text(&given.from.latest, &given.dir, LATEST_MOST);
        let release = latest.and_then(|json| Release::read(&json))?;
        crate::lock(&self.0.state).1 = Some(crate::now());
        // Not newer, or with no build for this system (a release of other systems alone): no
        // update.
        let Some(of) = release
            .size(name)
            .filter(|_| release.newer_than(&given.current))
        else {
            self.set(Stage::Idle);
            return Ok(());
        };
        let of = Some(of).filter(|of| *of > 0);
        if ready == Some(release.version) {
            return Ok(());
        }
        let fetching = |got: u64| Stage::Fetching {
            release: release.clone(),
            got,
            of,
        };
        self.set(fetching(0));
        let told = |got: u64| self.set(fetching(got));
        let staged = install::stage(&given.from, &release, name, &given.dir, &told)?;
        if let Err(why) = (given.trusted)(&staged) {
            let _ = std::fs::remove_dir_all(&given.dir);
            return Err(format!(
                "LLX Desk {} was fetched, but {why}",
                release.version
            ));
        }
        self.set(Stage::Ready { release });
        Ok(())
    }

    /// What is ready put in the app's place. Answers how the app is started again, for the
    /// caller to run as the app ends; the app that runs is the old one until it does.
    pub fn apply(&self) -> Result<Again, String> {
        let Stage::Ready { release } = crate::lock(&self.0.state).0.clone() else {
            return Err("No update is ready.".into());
        };
        let place = self.placed()?;
        if self.0.applying.swap(true, Ordering::SeqCst) {
            return Err(format!("LLX Desk {} is being installed.", release.version));
        }
        let name = self.0.given.archive.unwrap_or_default();
        let staged = (self.0.given.dir.join("unpacked")).join(install::unpacked(name));
        install::swap(&staged, &place).inspect_err(|_| {
            self.0.applying.store(false, Ordering::SeqCst);
            self.set(Stage::Failed {
                why: format!("LLX Desk {} could not be put in place.", release.version),
            });
        })?;
        Ok(Again {
            old: install::aside(&place),
            place,
        })
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
