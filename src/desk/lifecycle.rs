//! The app's own life as the person lives it: they quit it (asked first while programs run),
//! or bring it forward with nothing to show (their shell, in their home). The window only says
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

    /// The person brought the app forward: the window to them, with their shell in their home
    /// when it has no tab. On a thread of its own, as a quit is.
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
    /// quit; else their shell in its folder, Home's when there is none), with this process's
    /// environment (a login shell reads the rest of theirs).
    pub fn reopen(&self) -> Result<String, String> {
        let home = self.last_project()?;
        let owner = crate::projects::owner(&home, None);
        let vars: Vec<(String, String)> = std::env::vars().collect();
        self.present(
            &Call {
                owner: &owner,
                project: &home,
                agent: None,
                cwd: &home.path,
                vars: Some(&vars),
                peek: true,
                again: None,
                person: true,
                window: None,
            },
            false,
        )
    }

    /// How many terminals still run their program.
    fn running_programs(&self) -> usize {
        let sessions = crate::lock(&self.sessions);
        sessions.values().filter(|session| !session.ended()).count()
    }
}

/// What quitting would end, in words.
fn quit_detail(running: usize) -> String {
    let terminals = match running {
        1 => "A terminal is".to_string(),
        n => format!("{n} terminals are"),
    };
    format!("{terminals} still running a program. Quitting ends it, and the window's tabs close.")
}

#[cfg(test)]
mod tests {
    use super::quit_detail;

    #[test]
    fn the_question_counts_what_quitting_ends() {
        assert!(quit_detail(1).starts_with("A terminal is still running"));
        assert!(quit_detail(3).starts_with("3 terminals are still running"));
    }
}
