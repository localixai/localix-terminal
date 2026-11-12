//! What the browser tells the app, as plain values with nothing of the engine in them: a tab as
//! the agent reads it, and what the person did in the window.

/// Tabs open at once: each is a WebView with its own web process.
pub const MAX_TABS: usize = 8;

/// What the person did in the window, in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard<'a> {
    /// They did something in a tab (went to an address): news for whoever works in it.
    Did(&'a str),
    /// They closed a tab: news for whoever works in it.
    Closed(&'a str),
    /// They asked to quit the app (its menu, the Dock, or closing its main window): the desk
    /// decides, asking them first when programs run.
    Quit,
    /// They asked to restart the app into the update that is ready: the desk decides, asking
    /// them first when programs run.
    Update,
    /// They brought the app forward with nothing to show (its Dock icon, opening it again): the
    /// desk gives them a window, empty when nothing was left open.
    Reopened,
    /// They sent their comments on a page: words for the agent, as a message of theirs.
    Sent(&'a str),
    /// They asked for a tab of their own ("+"), in the project the key names (a group: the
    /// project of the tab shown, empty when it has none).
    Asked(Asked),
    /// What they did in the Event Center.
    Center(CenterDid),
    /// The window shows another project now (the key `heard` gives): the one used last.
    Showing,
    /// They picked a folder to open as a project (Open Folder…): the app makes it one, and the
    /// window goes to it.
    Folder(&'a std::path::Path),
    /// A page showed an address, or was titled: the ledger keeps it (popups too, which the
    /// window opened itself).
    Went(Went<'a>),
    /// The window split in the project of this key (its halves' tabs): the layout keeps it.
    Split(Halves<'a>),
    /// The split of these two tabs made whole, or one of them gone: the layout keeps their tab
    /// whole, and every other split as it is.
    Whole(Halves<'a>),
    /// They finished resizing this pane's split: the left half's share, in ten-thousandths.
    SplitResized(u16),
    /// They dragged the tab of this key's pane before the tab of pane `before` (none: to the
    /// end of its project's list): the layout keeps it.
    Moved {
        before: Option<&'a str>,
        /// The window whose list it was let go over (its row in the layout), when not its own.
        window: Option<i64>,
    },
    /// They let tab `key` go away from every list: it is a window of its own now (docs/windows.md).
    Detached,
    /// They closed window `i64` (its row in the layout; not the main one): its tabs are closed
    /// already, and the layout closes it.
    WindowClosed(i64),
    /// What they did in the tree of files of the project this key names.
    Tree(TreeDid<'a>),
}

/// What the person did in a project's tree of files. A folder is named by its path under the
/// project's folder, its names joined by `/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeDid<'a> {
    /// A window shows the project's tree now, or none does any more: only a tree in sight is
    /// read and watched.
    Seen(bool),
    /// They opened this folder.
    Opened(&'a str),
    /// They shut this folder.
    Shut(&'a str),
}

/// News of a project's tree of files, as the window draws it: its folder, every folder open
/// in it now (by its path under the root, `""` the root itself), and those of them whose
/// entries are new to the window or changed, a parent before its own. What the window holds
/// of a folder no longer open, or of another root, it lets go of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub root: std::path::PathBuf,
    pub open: Vec<String>,
    pub dirs: Vec<Branch>,
}

/// One open folder of a tree: its path under the root (`""`: the root), its entries in
/// order, and how many more it holds than are listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub path: String,
    pub leaves: Vec<Leaf>,
    pub more: usize,
}

/// One entry of a folder: its name, and whether it is a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    pub name: String,
    pub dir: bool,
}

/// A project's tabs in the window as the layout keeps them (docs/windows.md): the window draws
/// them so, and keeps no order or split of its own.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arranged {
    /// The window (its row in the layout).
    pub window: i64,
    /// The project's key.
    pub group: String,
    /// Its tabs in their places.
    pub tabs: Vec<Arranging>,
    /// The pane in focus of the tab the workspace shows.
    pub shown: Option<String>,
}

/// One tab: its panes in sight by their keys, left to right (a split's two, or one), side by
/// side or not, each one's share, and the one in focus. `docs` are its documents (a session's
/// files and pages, one of them in sight at most) and `all` every pane it has, in order.
/// `whole`, the person keeps it to its pane in focus alone, the rest put by.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arranging {
    pub panes: Vec<String>,
    pub across: bool,
    pub shares: Vec<f64>,
    pub focused: Option<String>,
    pub docs: Vec<String>,
    pub all: Vec<String>,
    pub whole: bool,
}

/// A project's split window: the tab in use, the one beside it, and whether that one is on the
/// right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Halves<'a> {
    pub shown: &'a str,
    pub beside: &'a str,
    pub right: bool,
}

/// A question to the person: go on (`yes`) or Cancel. The desk words it; the window asks it as
/// its system asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    pub title: String,
    pub detail: String,
    pub yes: String,
}

/// Where the person's answers to the window's offers are kept (offer.rs asks; the app keeps
/// them, `offers::Kept`).
pub trait Answers: Send + Sync {
    /// Whether offer `id` may be shown now.
    fn due(&self, id: &str) -> bool;
    /// The person answered offer `id`: took it (`taken`), or put it off.
    fn answered(&self, id: &str, taken: bool);
}

/// What a page showed: its address and title, and the project its tab is of (empty: the
/// window does not know, a popup's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Went<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub group: &'a str,
}

/// What a tab of a project's past is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Page,
    Terminal,
    Board,
    File,
}

/// A tab of a project's past, as the history menu lists it: its row in the ledger, what it is,
/// its title and its page (or folder), and its tab's key while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Past {
    pub row: i64,
    pub kind: TabKind,
    pub title: String,
    pub detail: String,
    pub key: Option<String>,
}

/// A page a project showed, which the address field suggests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visited {
    pub title: String,
    pub url: String,
}

/// What the window holds of a project's past: its tabs, the latest first, and its pages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    pub tabs: Vec<Past>,
    pub visited: Vec<Visited>,
}

/// A project as the window shows it: the key its tabs are grouped by, what it is called (its
/// folder's name, told apart from another of the same name), and its folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub key: String,
    pub name: String,
    pub folder: std::path::PathBuf,
}

/// What the person did in the Event Center.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterDid {
    /// They opened it: all it shows is seen.
    Opened,
    /// They chose button `button` of the event numbered `event`.
    Chose { event: u64, button: usize },
    /// They cleared it: all it shows is dismissed.
    Cleared,
}

/// A tab the person asked for: with "+", its keys, the history, the address field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// A page of theirs, at this address, or with none on the search page. The browser opens
    /// for it when none is open.
    Page(Option<String>),
    /// A terminal of theirs, in this folder of the project, else in the project's.
    Terminal(Option<std::path::PathBuf>),
    /// A file of theirs in an editor's tab: its path. What is not text opens as a page.
    File(std::path::PathBuf),
    /// The project's board, opened or shown.
    Board,
    /// A coding agent's program, in a terminal of theirs in the project's folder: its command.
    Agent(&'static str),
    /// A coding agent started on a card of the board, in a terminal of theirs in the project's
    /// folder: its command line (its first prompt in it) and the terminal's title (`c3 · codex`).
    Worker {
        command: String,
        title: String,
        /// Its name on the board: who it is in its terminal.
        who: String,
    },
    /// A coding agent's session continued in a terminal of theirs, in the folder of the project
    /// it works in: the command line that continues it and the terminal's title. `waits` is
    /// whose wait in the Event Center it answers once the terminal opened.
    Session {
        command: String,
        title: String,
        folder: std::path::PathBuf,
        waits: String,
    },
    /// A closed tab of the project's history, opened again: its row in the ledger.
    Again(i64),
    /// They went to the project, which has no tab open: what it had open when the app quit, or
    /// their shell in its folder.
    Project,
    /// A new window, on the project, with their shell in its folder.
    Window,
}

/// One tab as the agent reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: u32,
    pub url: String,
    pub title: String,
}

/// What the person's closing a tab of `kind` (a key's) says, to whoever listens.
pub(super) fn closed(kind: &str) -> &'static str {
    match kind {
        "web" => "The person closed this browser tab.",
        "board" => {
            "The person closed the board's tab: the board is kept, and `board` shows it again."
        }
        "file" => {
            "The person closed this file's tab: text left unsaved in it is kept with the file."
        }
        _ => "The person closed this tab: its program runs on, out of sight.",
    }
}
