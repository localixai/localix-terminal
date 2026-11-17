//! What a shell said of its commands as the core read it (OSC 133, which reaches it with the rest
//! of the output): the last step it reported (a prompt began, input began, output began, the command
//! ended and with what code), whether the cursor is at a prompt, and what each cell in view holds
//! (the prompt, what was typed at it, or a command's output).

use std::ffi::c_void;

use ghostty_vt_sys as vt;

use super::effects::Heard;
use super::{Core, Refused, done};

/// A step of a command, as the shell reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A prompt is drawn (`A`).
    Prompt,
    /// What follows is typed at it (`B`).
    Input,
    /// The command runs, and what follows is its output (`C`).
    Output,
    /// The command ended (`D`), with its exit code when the shell gave one.
    Ended(Option<i32>),
}

/// What a cell holds, as the shell's marks divide the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    Prompt,
    Input,
    Output,
}

/// What the core calls as the shell reports a step: kept in the `Heard` it was given.
unsafe extern "C" fn heard(
    _terminal: vt::Terminal,
    userdata: *mut c_void,
    event: *const vt::TerminalSemanticPrompt,
) {
    // SAFETY: `userdata` is the core's `Heard`, which outlives its terminal; null is checked.
    let said = unsafe { userdata.cast::<Heard>().as_ref() };
    // SAFETY: the event is the library's, valid through the call; null is checked.
    let event = unsafe { event.as_ref() };
    let (Some(said), Some(event)) = (said, event) else {
        return;
    };
    let step = match event.kind {
        vt::SemanticPromptKind::PROMPT_START => Step::Prompt,
        vt::SemanticPromptKind::INPUT_START => Step::Input,
        vt::SemanticPromptKind::OUTPUT_START => Step::Output,
        vt::SemanticPromptKind::COMMAND_END => {
            Step::Ended(event.has_exit_code.then_some(event.exit_code))
        }
        _ => return,
    };
    Core::hear_step(said, step);
}

/// Has the core tell its record each step the shell reports (its user data is set already).
pub(super) fn listen(core: &Core) -> Result<(), Refused> {
    let callback: vt::TerminalSemanticPromptFn = Some(heard);
    let callback = callback.map_or(std::ptr::null(), |heard| heard as *const c_void);
    // SAFETY: a callback is handed over as the function itself, of the type the option names.
    let set = unsafe {
        vt::ghostty_terminal_set(core.term, vt::TerminalOption::SEMANTIC_PROMPT, callback)
    };
    done(set, "to hear the shell's marks")
}

impl Core {
    /// Whether the cursor is at the shell's prompt or what is typed at it.
    pub fn at_prompt(&self) -> bool {
        self.get(vt::TerminalData::CURSOR_AT_PROMPT)
    }

    /// What the cell at `row` and `column` of the rows in view holds, from 0; none off the grid.
    pub fn content(&self, row: u16, column: u16) -> Option<Content> {
        let at = self.cell(row, column)?;
        let mut cell: vt::Cell = 0;
        // SAFETY: `at` was read just now, the terminal unchanged since; `cell` is written.
        let read = unsafe { vt::ghostty_grid_ref_cell(&raw const at, &raw mut cell) };
        done(read, "a cell").ok()?;
        let mut content: vt::CellSemanticContent::Type = vt::CellSemanticContent::OUTPUT;
        let out: *mut vt::CellSemanticContent::Type = &raw mut content;
        let data = vt::CellData::SEMANTIC_CONTENT;
        // SAFETY: the cell's content is written as the type `out` points to.
        let read = unsafe { vt::ghostty_cell_get(cell, data, out.cast()) };
        done(read, "a cell").ok()?;
        Some(match content {
            vt::CellSemanticContent::PROMPT => Content::Prompt,
            vt::CellSemanticContent::INPUT => Content::Input,
            _ => Content::Output,
        })
    }
}
