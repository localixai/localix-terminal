//! Opening tabs: one or several at once, each a page, a terminal, the board or a file, each owned
//! from the moment it exists.

use std::sync::Arc;

use serde_json::Value;

use super::actions::Kind;
use super::args::text;
use super::ledger::Facts;
use super::{Call, Desk, Failure, Output};

/// The most tabs one `open` makes: a call may open every page, every terminal and the board.
const MOST_AT_ONCE: usize = 32;

/// The fields `open` reads for a tab of `kind`: the address of a page; a terminal's program, where
/// it starts and how long to wait for its output; none for the board; a file's path and the line
/// to show.
fn open_fields(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Web => &["url"],
        Kind::Term => &["command", "workdir", "wait_for", "yield_ms"],
        Kind::Board | Kind::Made(_) => &[],
        Kind::File => &["path", "line"],
    }
}

/// The kind of tab `spec` asks for; `kinds` are those that draw themselves.
fn kind_of(spec: &Value, kinds: &[Arc<dyn crate::kind::Kind>]) -> Result<Kind, String> {
    Ok(match text(spec, "kind") {
        Some("browser") => Kind::Web,
        Some("terminal") => Kind::Term,
        Some("board") => Kind::Board,
        Some("file") => Kind::File,
        Some(other) => match kinds.iter().find(|kind| kind.about().id == other) {
            Some(made) => Kind::Made(made.about()),
            None => {
                let mut known = vec!["browser", "terminal", "board", "file"];
                known.extend(kinds.iter().map(|kind| kind.about().id));
                return Err(format!("no kind of tab `{other}`: {}", known.join(", ")));
            }
        },
        None if text(spec, "command").is_some() => Kind::Term,
        None if text(spec, "path").is_some() => Kind::File,
        None => Kind::Web,
    })
}

/// `spec` is a tab `open` can make: an object, naming a kind of tab. A field no call has (a typo
/// in a terminal's `command` inside `tabs`) is refused, never dropped: it opened the search page,
/// and read as a page that would not do as told. A field of the tool that `open` does not read
/// (`size`, `ask`) is let be, as it always was.
fn opened_as_asked(spec: &Value, kinds: &[Arc<dyn crate::kind::Kind>]) -> Result<(), String> {
    let given = spec
        .as_object()
        .ok_or("each of `tabs` is an object: `kind` and its fields")?;
    let kind = kind_of(spec, kinds)?;
    let takes = open_fields(kind);
    let stray: Vec<String> = (given.iter())
        .filter(|(_, value)| !value.is_null())
        .map(|(field, _)| field.as_str())
        .filter(|field| !super::args::is_field(field))
        .map(|field| format!("`{field}`"))
        .collect();
    if stray.is_empty() {
        return Ok(());
    }
    let listed = match takes {
        [] => "no field".to_string(),
        _ => (takes.iter())
            .map(|field| format!("`{field}`"))
            .collect::<Vec<_>>()
            .join(", "),
    };
    Err(format!(
        "a {} tab is opened with no {}: nothing was opened. It takes {listed}.",
        kind.name(),
        stray.join(", ")
    ))
}

impl Desk {
    /// Open each tab asked for; one that fails says why, and the others still open.
    #[expect(
        clippy::needless_collect,
        reason = "every tab's thread starts before the first is joined"
    )]
    pub(super) fn open(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        let asked: Vec<&Value> = match args.get("tabs").and_then(Value::as_array) {
            Some(tabs) => tabs.iter().collect(),
            None => vec![args],
        };
        if asked.len() > MOST_AT_ONCE {
            return Err(format!("open at most {MOST_AT_ONCE} tabs at once").into());
        }
        for spec in &asked {
            opened_as_asked(spec, &self.made.kinds)?;
        }
        let opened: Vec<Result<(String, Output), Failure>> = std::thread::scope(|scope| {
            let opening: Vec<_> = asked
                .iter()
                .map(|spec| {
                    let thread = std::thread::Builder::new().name("desk-tab-open".into());
                    thread.spawn_scoped(scope, move || self.open_one(call, spec))
                })
                .collect();
            let joined = opening.into_iter().map(|started| match started {
                Ok(thread) => thread
                    .join()
                    .unwrap_or_else(|_| Err("opening it failed".to_string().into())),
                Err(why) => Err(format!("it could not start (no thread: {why})").into()),
            });
            joined.collect()
        });
        let (mut reports, mut states, mut shot) = (vec![], vec![], None);
        for opened in opened {
            match opened {
                Ok((label, out)) => {
                    reports.push(format!("{label}: {}", out.report));
                    if let Some(state) = out.state {
                        states.push(format!("### {label}\n{state}"));
                    }
                    shot = out.shot.or(shot);
                }
                Err(why) => reports.push(format!("not opened: {}", why.message())),
            }
        }
        Ok(Output {
            report: reports.join("\n"),
            state: (!states.is_empty()).then(|| states.join("\n\n")),
            shot,
        })
    }

    /// One tab of an `open`, owned from the moment it exists.
    fn open_one(&self, call: &Call, spec: &Value) -> Result<(String, Output), Failure> {
        let kind = kind_of(spec, &self.made.kinds)?;
        let (id, out) = match kind {
            Kind::Web => return self.open_page(call, spec),
            Kind::Term => self.open_term(call, spec)?,
            // The owner's one board, shown: opened the first time, the same tab after.
            Kind::Board => {
                let out = self.board(call, &serde_json::json!({"op": "show"}))?;
                return Ok((kind.name().to_string(), out));
            }
            // The project's one tab of a kind that draws itself, the same way.
            Kind::Made(about) => {
                let out = self.made(call, about, &serde_json::json!({}))?;
                return Ok((kind.name().to_string(), out));
            }
            // A file's one tab in the project, shown: opened the first time, the same tab after.
            Kind::File => {
                let path = text(spec, "path").ok_or_else(|| {
                    "a file tab is opened with `path`: the file to edit".to_string()
                })?;
                let line = (super::args::number(spec, "line")).and_then(|n| u32::try_from(n).ok());
                let key = (self.open_file(call, std::path::Path::new(path), line))
                    .map_err(|not| not.why)?;
                let out = Output::from(self.file_summary(&key));
                return Ok((format!("{key} ({})", kind.name()), out));
            }
        };
        let Some(key) = id.map(|id| format!("t{id}")) else {
            // The program ended within the wait: no tab, and what it printed.
            return Ok((kind.name().to_string(), out));
        };
        let dir = super::term::term_dir(call, spec);
        let facts = Facts {
            title: text(spec, "command").unwrap_or_default(),
            cwd: Some(&dir),
            command: text(spec, "command"),
            ..Facts::default()
        };
        self.own(&key, call, crate::ledger::Kind::Terminal, facts);
        Ok((format!("{key} ({})", kind.name()), out))
    }

    /// A page's tab: owned, and in its session, as soon as the window has made it, and read
    /// once its page has loaded. What the person opens they read themselves: nothing waits
    /// for its load.
    fn open_page(&self, call: &Call, spec: &Value) -> Result<(String, Output), Failure> {
        let mark = self.told_mark();
        let (id, url) = self.start_web(call, spec)?;
        let key = format!("w{id}");
        let facts = Facts {
            url: text(spec, "url"),
            ..Facts::default()
        };
        self.own(&key, call, crate::ledger::Kind::Page, facts);
        if let Some(browser) = &self.browser {
            browser.group(&key, &crate::projects::key(call.project.id));
        }
        let label = format!("{key} ({})", Kind::Web.name());
        if call.person {
            return Ok((label, Output::from(format!("Opened {url}."))));
        }
        let mut out = self.read_new(id, &url)?;
        if let Some(told) = self.told(id, mark) {
            out.report = format!("{}\n{told}", out.report);
        }
        Ok((label, out))
    }
}
