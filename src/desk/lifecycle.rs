//! The app's own life as the person lives it: they quit it (asked first while programs run),
//! open it from a folder (the window comes to them, with what the project had open), or bring
//! it forward with nothing to show (their shell, in their home). The window only says
//! what they did (`Heard::Quit`, `Heard::Reopened`); what that comes to is decided here, and
//! the end itself is the host's (`Desk::on_quit`: the app stops).

use std::sync::Arc;

use super::{Call, Desk};
use crate::browser::Confirm;

impl Desk {
    /// What ends the app when the person quits it: given once, by whoever runs the desk.
    pub fn on_quit(&self, quit: impl Fn() + Send + Sync + 'static) {
        let _ = self.quit.set(Box::new(quit));
    }

    /// The person asked to quit: asked first when terminals still run a program, then ended.
    /// On a thread of its own: the window's loop calls this, and the question waits on it.
    pub(super) fn quit_asked(self: &Arc<Self>) {
        let desk = Arc::clone(self);
        let asked = std::thread::Builder::new()
            .name("desk-quit".into())
            .spawn(move || {
                let running = desk.running_programs();
                let sure = running == 0
                    || (desk.browser.as_ref()).is_some_and(|browser| {
                        browser.confirm(&Confirm {
                            title: "Quit LLX Desk?".into(),
                            detail: quit_detail(running),
                            yes: "Quit".into(),
                        })
                    });
                if sure && let Some(quit) = desk.quit.get() {
                    quit();
                }
            });
        if let Err(why) = asked {
            log::warn!("desk: the quit was not asked (no thread: {why})");
        }
    }

    /// The person asked to restart into the update that is ready: asked first when terminals
    /// still run a program; then the new app is put in place, set to open once this one ended,
    /// and this one ends as a quit ends it. On a thread of its own, as a quit is.
    pub(super) fn update_asked(self: &Arc<Self>) {
        let desk = Arc::clone(self);
        let asked = std::thread::Builder::new()
            .name("desk-update".into())
            .spawn(move || {
                let running = desk.running_programs();
                let sure = running == 0
                    || (desk.browser.as_ref()).is_some_and(|browser| {
                        browser.confirm(&Confirm {
                            title: "Restart LLX Desk to update?".into(),
                            detail: update_detail(running),
                            yes: "Restart".into(),
                        })
                    });
                if !sure {
                    return;
                }
                let updated = crate::update::app().apply();
                match updated.and_then(|again| again.start()) {
                    Ok(()) => {
                        if let Some(quit) = desk.quit.get() {
                            quit();
                        }
                    }
                    // Settings says so: the updater told it.
                    Err(why) => log::warn!("desk: not updated: {why}"),
                }
            });
        if let Err(why) = asked {
            log::warn!("desk: the update was not asked (no thread: {why})");
        }
    }

    /// The person brought the app forward: the window to them, empty when nothing was left
    /// open. On a thread of its own, as a quit is.
    pub fn reopened(self: &Arc<Self>) {
        let desk = Arc::clone(self);
        let opened = std::thread::Builder::new()
            .name("desk-reopen".into())
            .spawn(move || {
                if let Err(why) = desk.reopen() {
                    log::warn!("desk: the window did not open: {why}");
                }
            });
        if let Err(why) = opened {
            log::warn!("desk: the window was not opened (no thread: {why})");
        }
    }

    /// The window to the person, as the app opened from the Finder or its Dock icon gives it:
    /// when it has no tab, the project used last as it was left (its tabs open when the app
    /// quit; else nothing, the window's start of a new tab), with this process's environment (a
    /// login shell reads the rest of theirs).
    pub fn reopen(&self) -> Result<String, String> {
        let home = self.last_project()?;
        let owner = crate::projects::owner(&home, None);
        self.present(
            &Call {
                owner: &owner,
                project: &home,
                agent: None,
                cwd: &home.path,
                vars: None,
                peek: true,
                again: None,
                person: true,
                window: None,
                from: None,
                runs: None,
            },
            false,
        )
    }

    /// The person opened the app (app.rs): the window comes to them, back if they put it away;
    /// with no tab in it yet, what the project had open, else the window's start of a new tab.
    pub fn present(&self, call: &Call, go: bool) -> Result<String, String> {
        let browser = (self.browser.as_ref()).ok_or("there is no window here: no display")?;
        crate::lock(&self.registry).known(call);
        let group = crate::projects::key(call.project.id);
        let listed = browser.listing().unwrap_or_default();
        // Going to the project: the window goes first, so what opens for it opens in sight.
        let empty = match go {
            true => {
                browser.go(&group);
                !listed.iter().any(|tab| tab.group == group)
            }
            false => listed.is_empty(),
        };
        if empty {
            self.reopen_project(call, &group)?;
        }
        browser.present();
        Ok(if empty {
            "The window opened, with what the project had open.".into()
        } else {
            "The window opened.".into()
        })
    }

    /// How many terminals still run their program.
    fn running_programs(&self) -> usize {
        let sessions = crate::lock(&self.sessions);
        sessions.values().filter(|session| !session.ended()).count()
    }
}

/// How many terminals still run a program, in words.
fn terminals(running: usize) -> String {
    match running {
        1 => "A terminal is".to_string(),
        n => format!("{n} terminals are"),
    }
}

/// What quitting would end, in words.
fn quit_detail(running: usize) -> String {
    let terminals = terminals(running);
    format!("{terminals} still running a program. Quitting ends it, and the window's tabs close.")
}

/// What restarting into an update would end, in words.
fn update_detail(running: usize) -> String {
    let terminals = terminals(running);
    format!(
        "{terminals} still running a program. Restarting ends it; the window opens again with \
         the tabs it had."
    )
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
