//! Short commands: `llxterm click e12` is one `window` action in a few words, and `+` joins
//! several into one call. The verbs are one table, which builds each step's JSON and is printed
//! as `--help`, as the guide's `commands` topic and in every usage error, so the short form and
//! the JSON never drift apart. It knows nothing of the crate: the command line sends what it
//! builds, and the desk reads a step given as a string through it too.

mod build;
mod words;

use serde_json::Value;

use build::{Given, Tab};
pub use words::words;

/// One verb: its name, how it is written, what it does, and how its words become an action.
pub struct Verb {
    name: &'static str,
    /// How the command is written, after `llxterm`.
    synopsis: &'static str,
    /// What it does, in a few words.
    says: &'static str,
    tab: Tab,
    /// The words it needs after its tab, at least.
    needs: usize,
    /// The flags it takes; a name ending in `=` is followed by a value.
    flags: &'static [&'static str],
    build: fn(&mut Given) -> Result<Value, String>,
}

/// The flags of a step on a page: how its dialogs are answered.
const PAGE: [&str; 2] = ["dialog=", "answer="];

/// Every verb, each one `window` action.
#[rustfmt::skip]
pub const VERBS: [Verb; 19] = [
    verb("open", "open URL|FILE", "a page: an address (localhost:3000) or an HTML file", Tab::No, 1, &PAGE, build::open),
    verb("term", "term [CMD] [--in DIR] [--wait TEXT] [--ms N] [--beside TAB] [--down]", "a terminal running CMD (your shell with none), returned once TEXT shows", Tab::No, 0, &["in=", "wait=", "ms=", "beside=", "down"], build::term),
    verb("edit", "edit PATH [--line N]", "a text file of the project in the editor", Tab::No, 1, &["line="], build::edit),
    verb("goto", "goto [TAB] URL", "another address in a page", Tab::Maybe, 1, &PAGE, build::goto),
    verb("click", "click [TAB] REF…", "click controls in order", Tab::Maybe, 1, &PAGE, build::click),
    verb("fill", "fill [TAB] REF TEXT [REF TEXT]…", "type TEXT into fields", Tab::Maybe, 1, &PAGE, build::fill),
    verb("select", "select [TAB] REF OPTION", "pick an option", Tab::Maybe, 2, &PAGE, build::select),
    verb("press", "press [TAB] KEY… [--delay MS]", "press keys: Enter, Tab, Ctrl+A, Ctrl-C", Tab::Maybe, 1, &["delay=", "dialog=", "answer="], build::press),
    verb("type", "type [TAB] TEXT [--enter]", "type into a terminal; --enter submits it", Tab::Maybe, 1, &["enter"], build::type_in),
    verb("wait", "wait [TAB] TEXT [--ms N]", "until TEXT shows, at most N ms", Tab::Maybe, 1, &["ms=", "dialog=", "answer="], build::wait),
    verb("read", "read [TAB] [--text [--from N]]", "the tab as it is; --text adds a page's whole text", Tab::Maybe, 0, &["text", "from=", "dialog=", "answer="], build::read),
    verb("eval", "eval [TAB] JS|-", "run JavaScript in a page and get its value", Tab::Maybe, 1, &PAGE, build::eval),
    verb("size", "size [TAB] SIZE", "a page's size: iPhone SE, mobile, tablet, 393x852, window", Tab::Maybe, 1, &PAGE, build::size),
    verb("ask", "ask [TAB] TEXT", "ask the person for what only they can do", Tab::Maybe, 1, &[], build::ask),
    verb("shot", "shot [TAB] [--of CSS]", "a picture of a page, or of what CSS matches", Tab::Maybe, 0, &["of="], build::shot),
    verb("show", "show TAB [--zoom|--unzoom]", "a tab in front of the person; --zoom fills its tab", Tab::Must, 0, &["zoom", "unzoom"], build::show),
    verb("close", "close TAB…", "end tabs", Tab::No, 1, &[], build::close),
    verb("list", "list", "your tabs and where they lie", Tab::No, 0, &[], build::list),
    verb("do", "do - | do STEP…", "steps a line each from stdin, or an argument each", Tab::No, 0, &[], build::nested),
];

/// A row of the table.
const fn verb(
    name: &'static str,
    synopsis: &'static str,
    says: &'static str,
    tab: Tab,
    needs: usize,
    flags: &'static [&'static str],
    build: fn(&mut Given) -> Result<Value, String>,
) -> Verb {
    Verb {
        name,
        synopsis,
        says,
        tab,
        needs,
        flags,
        build,
    }
}

impl Verb {
    /// The command's word, after `llxterm`.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// How the command is written, after `llxterm`.
    pub const fn synopsis(&self) -> &'static str {
        self.synopsis
    }
}

/// Whether `name` is a verb of the table.
pub fn is_verb(name: &str) -> bool {
    VERBS.iter().any(|verb| verb.name == name)
}

/// The steps `argv` (the words after `llxterm`, a verb first) asks for, `+` between them; `do`
/// takes them as strings, `do -` a line each from `stdin`, which also gives a `-` text.
pub fn steps(
    argv: &[String],
    stdin: &mut dyn FnMut() -> Result<String, String>,
) -> Result<Vec<Value>, String> {
    if let Some((first, rest)) = argv.split_first()
        && first == "do"
    {
        return match rest {
            [one] if one == "-" => lines(&stdin()?),
            [] => Err("do: needs steps: llxterm do - | do STEP…".into()),
            given => numbered(given.iter().map(|step| line(step)), given.len()),
        };
    }
    let split = words::split(argv)?;
    numbered(split.iter().map(|words| step(words, stdin)), split.len())
}

/// The steps of a text, one a line; blank lines and `#` comments are passed over.
pub fn lines(text: &str) -> Result<Vec<Value>, String> {
    let given: Vec<&str> = (text.lines().map(str::trim))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    if given.is_empty() {
        return Err("stdin holds no steps: one a line".into());
    }
    numbered(given.iter().map(|text| line(text)), given.len())
}

/// One step written as a string, split as a shell splits words (with no substitution).
pub fn line(text: &str) -> Result<Value, String> {
    let words = words(text)?;
    step(&words, &mut || {
        Err("`-` reads stdin, which a step given as a string does not have".into())
    })
}

/// Each of `count` steps, or the first that is wrong, numbered when there are several.
fn numbered(
    steps: impl Iterator<Item = Result<Value, String>>,
    count: usize,
) -> Result<Vec<Value>, String> {
    (steps.enumerate())
        .map(|(at, step)| match count {
            1 => step,
            _ => step.map_err(|why| format!("step {}: {why}", at + 1)),
        })
        .collect()
}

/// One step from its words, its verb first.
fn step(
    words: &[String],
    stdin: &mut dyn FnMut() -> Result<String, String>,
) -> Result<Value, String> {
    let (name, rest) = words.split_first().ok_or("a step is empty")?;
    let verb = (VERBS.iter().find(|verb| verb.name == name)).ok_or_else(|| unknown(name))?;
    let mut given = Given::read(verb, rest, stdin)?;
    (verb.build)(&mut given)
}

/// No verb `name`: the nearest, and every one.
fn unknown(name: &str) -> String {
    let names: Vec<&str> = VERBS.iter().map(|verb| verb.name).collect();
    let near = (names.iter())
        .map(|verb| (words::distance(verb, name), verb))
        .filter(|(far, _)| *far <= 2)
        .min_by_key(|(far, _)| *far)
        .map(|(_, verb)| verb);
    let guess = near.map_or_else(String::new, |verb| format!(", did you mean `{verb}`?"));
    format!(
        "no command `{name}`{guess} The commands: {}",
        names.join(", ")
    )
}

/// The commands, a line each, as `--help` lists them.
pub fn help() -> String {
    (VERBS.iter())
        .map(|verb| format!("  llxterm {:<44} {}", verb.synopsis, verb.says))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The guide's topic of the short commands: the table, then how a step finds its tab and how
/// steps run together.
pub fn guide() -> String {
    let listed: Vec<String> = (VERBS.iter())
        .map(|verb| format!("- `llxterm {}`: {}.", verb.synopsis, verb.says))
        .collect();
    let (lead, rules) = (
        include_str!("short/lead.md"),
        include_str!("short/rules.md"),
    );
    format!(
        "{}\n{}\n\n{}",
        lead.trim_end(),
        listed.join("\n"),
        rules.trim_end()
    )
}

#[cfg(test)]
#[path = "short_tests.rs"]
mod tests;
