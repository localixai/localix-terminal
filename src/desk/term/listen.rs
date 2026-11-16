//! What a terminal's session tells the desk and the window: its program's end and its notes,
//! news for the Event Center; the title it sets, under the tab's name; the program in front, the
//! tab's mark, and the coding agent it is, kept for the terminal (agent_resume.rs); and what the
//! program says of its state (OSC 7501), news for the Event Center and, while it is at work, a
//! still ring on the tab's icon; and each command the person typed in it that ended, which the
//! window tells them of when they were not looking.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use crate::browser::Working;
use crate::terminal::{Doing, Session};

use super::super::Desk;

impl Desk {
    /// Session `id`, started in `dir`, heard from here on.
    pub(super) fn hear_session(&self, id: u32, session: &Arc<Session>, dir: &std::path::Path) {
        // Its program's end is news for the Event Center, and closes its row of the ledger (its
        // last screen kept), on a thread of its own: the session's reader calls this.
        let (feed, me) = (self.feed.clone(), Weak::clone(&self.me));
        session.on_end(move || {
            feed.record(
                &crate::feed::Entity::Terminal { id },
                crate::feed::Op::Upserted,
            );
            let me = Weak::clone(&me);
            let closed = std::thread::Builder::new()
                .name("desk-terminal-ended".into())
                .spawn(move || {
                    if let Some(desk) = me.upgrade() {
                        desk.unkeep(&format!("t{id}"));
                    }
                });
            if let Err(why) = closed {
                log::warn!("terminal t{id} ended, still open in the ledger (no thread: {why})");
            }
        });
        // What its program asks to tell the person is news for the Event Center too.
        let feed = self.feed.clone();
        session.on_note(move || {
            feed.record(
                &crate::feed::Entity::Terminal { id },
                crate::feed::Op::Upserted,
            );
        });
        // What its program says it is doing (the title it sets) shows under the tab's name, and
        // which program is in front (a coding agent typed in its shell among them) is the window's
        // to know: its mark on the tab. The agent in front is kept for the terminal, window or
        // none. Where it is (the bar's crumbs) is asked again as its shell prompts (it sets the
        // title) and as another program comes in front.
        let (browser, me, there) = (
            self.browser.clone(),
            Weak::clone(&self.me),
            Arc::downgrade(session),
        );
        let agent_ran = Arc::new(AtomicBool::new(false));
        let ran = Arc::clone(&agent_ran);
        session.on_front(move |program| {
            if program.is_some_and(crate::browser::agent_runs) {
                ran.store(true, Ordering::SeqCst);
            }
            if let Some(browser) = &browser {
                browser.runs(&format!("t{id}"), program);
                if let Some(session) = there.upgrade() {
                    browser.folder(&format!("t{id}"), session.folder());
                }
            }
            if let Some(desk) = me.upgrade() {
                desk.agent_fronted(id);
            }
        });
        if let Some(browser) = self.browser.clone() {
            let (told, me) = (browser.clone(), Weak::clone(&self.me));
            let here = Arc::downgrade(session);
            browser.folder(
                &format!("t{id}"),
                session.folder().or_else(|| Some(dir.into())),
            );
            session.on_title(move |title| {
                told.detail(&format!("t{id}"), Some(title));
                if let Some(session) = here.upgrade() {
                    told.folder(&format!("t{id}"), session.folder());
                }
                if let Some(desk) = me.upgrade() {
                    desk.retitled(&format!("t{id}"), title);
                }
            });
        }
        self.listen_status(id, session);
        // A command the person typed that ended is the window's to weigh: news for them if they
        // were not looking. A coding agent's own end is not: its status says it (OSC 7501).
        if let Some(browser) = self.browser.clone() {
            session.on_command(move |ended, theirs| {
                let agent = agent_ran.swap(false, Ordering::SeqCst);
                if theirs && !agent {
                    browser.command_ended(id, ended.code, ended.took);
                }
            });
        }
    }

    /// What the program of session `id` says of its state: a change of its lines is news for
    /// the Event Center, which reads them again, told once until it has (a program that flips
    /// its state cannot flood the feed); the tab is marked at work while the record that wants
    /// the person most is one at work (what waits for them is the center's dot), told to the
    /// window only when that mark changes.
    fn listen_status(&self, id: u32, session: &Arc<Session>) {
        let (feed, browser) = (self.feed.clone(), self.browser.clone());
        let weak = Arc::downgrade(session);
        let marked = std::sync::Mutex::new(None::<Option<Working>>);
        session.on_status(move || {
            let Some(session) = weak.upgrade() else {
                return;
            };
            if session.status_news() {
                feed.record(
                    &crate::feed::Entity::Terminal { id },
                    crate::feed::Op::Upserted,
                );
            }
            let Some(browser) = &browser else {
                return;
            };
            let working = (session.most_urgent_status())
                .filter(|status| status.state == Doing::Working)
                .map(|status| Working {
                    progress: status.progress,
                });
            let mut marked = crate::lock(&marked);
            if marked.as_ref() != Some(&working) {
                *marked = Some(working);
                drop(marked);
                browser.working(&format!("t{id}"), working);
            }
        });
    }
}
