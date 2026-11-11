//! A call's context, errors and answer: the boundary that masks and renders tool output.

use std::path::Path;

use serde_json::Value;

use crate::browser::Shot;
use crate::projects::Project;

use super::Desk;

/// Who is calling, from where, and with what the call may need of the caller.
#[derive(Clone, Copy)]
pub struct Call<'a> {
    /// The caller's key (`projects::owner`): its tabs and notes are kept by it.
    pub owner: &'a str,
    /// The project it works in: everything it opens is the project's.
    pub project: &'a Project,
    /// Its name within the project; none: the project's own agent.
    pub agent: Option<&'a str>,
    /// The caller's directory: relative paths start here.
    pub cwd: &'a Path,
    /// The caller's environment, for a program started in a terminal.
    pub vars: Option<&'a [(String, String)]>,
    /// A look for a pane, not the agent's call: the person's notes stay for the agent.
    pub peek: bool,
    /// The pane this call opens again after a restart: its row, which it stays (its tab, its
    /// place and its split with it).
    pub again: Option<crate::ledger::Row>,
    /// The person asked for it in the window (a tab of theirs, or their tabs opened again): what
    /// it opens is theirs, and their closing it is news to no agent.
    pub person: bool,
    /// The window it opens in (its row in the layout); none: the window the person works in.
    pub window: Option<crate::layout::WindowId>,
    /// The terminal of the window its program runs in (`t3`), when it runs in one: what it
    /// opens lands beside that terminal (beside.rs).
    pub from: Option<&'a str>,
    /// The coding agent's program the call comes from (`claude`) and the folder that program
    /// works in, when the call descends from one: how its session is continued in a terminal
    /// of the window (center/leads.rs).
    pub runs: Option<(&'a str, &'a Path)>,
}

/// Why a call did not come to what it asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// This call failed; the next may not.
    Failed(String),
}

impl Failure {
    pub fn message(&self) -> &str {
        match self {
            Self::Failed(why) => why,
        }
    }
}

impl From<String> for Failure {
    fn from(why: String) -> Self {
        Self::Failed(why)
    }
}

/// What a call gives back: what happened, the tab as it is now, and maybe a picture of it.
#[derive(Default)]
pub struct Output {
    pub report: String,
    pub state: Option<String>,
    pub shot: Option<Shot>,
}

impl From<String> for Output {
    fn from(report: String) -> Self {
        Self {
            report,
            ..Self::default()
        }
    }
}

/// A call's answer before it is put into words: the picture is bytes still.
pub struct Answered {
    /// The report and the tab as it is now.
    pub text: String,
    /// What the person sent from the window, as the agent reads it.
    pub said: Option<String>,
    pub shot: Option<Shot>,
}

impl Answered {
    /// The text, a line about the picture if there is one, and what the person sent.
    pub fn text_with(&self, picture: Option<&str>) -> String {
        let parts = [Some(self.text.as_str()), picture, self.said.as_deref()];
        let kept: Vec<&str> = parts
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .collect();
        kept.join("\n\n").trim().to_string()
    }
}

/// `text` with its control characters left out but its line breaks and tabs: what a page or a
/// program wrote there (an escape sequence in a title) is shown as words, and never acts on the
/// terminal that prints it.
pub(super) fn plain(text: &str) -> String {
    (text.chars())
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

impl Desk {
    /// One `window` call, answered as text: the report, the tab, where a picture was saved, and
    /// what the person sent.
    pub fn window(&self, call: &Call, args: &Value) -> Result<String, Failure> {
        let answered = self.call(call, args)?;
        let picture = match &answered.shot {
            Some(shot) => {
                let path = crate::shots::save(&self.shots, &shot.jpeg).map_err(Failure::Failed)?;
                Some(format!(
                    "Screenshot saved ({}x{}): {}\nRead that file to see it.",
                    shot.width,
                    shot.height,
                    path.display()
                ))
            }
            None => None,
        };
        Ok(answered.text_with(picture.as_deref()))
    }

    /// One `window` call, answered with its picture as it is, for whoever carries bytes.
    pub fn call(&self, call: &Call, args: &Value) -> Result<Answered, Failure> {
        crate::lock(&self.registry).known(call);
        let out = self.act(call, args)?;
        let mut text = out.report;
        if let Some(state) = out.state {
            text = format!("{text}\n\n{state}");
        }
        Ok(Answered {
            text: self.vault.mask(plain(text.trim()).trim()),
            said: (!call.peek)
                .then(|| self.said(call.owner))
                .flatten()
                .map(|said| self.vault.mask(&plain(&said))),
            shot: out.shot,
        })
    }
}
