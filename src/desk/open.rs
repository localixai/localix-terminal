//! Opening tabs: one or several at once, each a page, a terminal or the board, each owned from the
//! moment it exists.

use serde_json::Value;

use super::actions::{Kind, parse};
use super::args::text;
use super::ledger::Facts;
use super::{Call, Desk, Failure, Output};

/// The most tabs one `open` makes: a call may open every page, every terminal and the board.
const MOST_AT_ONCE: usize = 32;

/// The fields `open` reads for a tab of `kind`: the address of a page; a terminal's program, where
/// it starts and how long to wait for its output; none for the board.
fn open_fields(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Web => &["url"],
        Kind::Term => &["command", "workdir", "wait_for", "yield_ms"],
        Kind::Board => &[],
    }
}

/// The kind of tab `spec` asks for.
fn kind_of(spec: &Value) -> Result<Kind, String> {
    Ok(match text(spec, "kind") {
        Some("browser") => Kind::Web,
        Some("terminal") => Kind::Term,
        Some("board") => Kind::Board,
        Some(other) => {
            return Err(format!(
                "no kind of tab `{other}`: browser, terminal or board"
            ));
        }
        None if text(spec, "command").is_some() => Kind::Term,
        None => Kind::Web,
    })
}

/// `spec` is a tab `open` can make: an object, naming a kind of tab. A field no call has (a typo
/// in a terminal's `command` inside `tabs`) is refused, never dropped: it opened a blank page,
/// and read as a page that would not do as told. A field of the tool that `open` does not read
/// (`size`, `ask`) is let be, as it always was.
fn opened_as_asked(spec: &Value) -> Result<(), String> {
    let given = spec
        .as_object()
        .ok_or("each of `tabs` is an object: `kind` and its fields")?;
    let kind = kind_of(spec)?;
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
            opened_as_asked(spec)?;
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
        let kind = kind_of(spec)?;
        let mark = self.told_mark();
        let (key, mut out) = match kind {
            Kind::Web => {
                let (id, out) = self.open_web(call, spec)?;
                (Some(format!("w{id}")), out)
            }
            Kind::Term => {
                let (id, out) = self.open_term(call, spec)?;
                (id.map(|id| format!("t{id}")), out)
            }
            // The owner's one board, shown: opened the first time, the same tab after.
            Kind::Board => {
                let out = self.board(call, &serde_json::json!({"op": "show"}))?;
                return Ok((kind.name().to_string(), out));
            }
        };
        let Some(key) = key else {
            // The program ended within the wait: no tab, and what it printed.
            return Ok((kind.name().to_string(), out));
        };
        let dir = super::term::term_dir(call, spec);
        let facts = match kind {
            Kind::Web => Facts {
                url: text(spec, "url"),
                ..Facts::default()
            },
            Kind::Term | Kind::Board => Facts {
                title: text(spec, "command").unwrap_or_default(),
                cwd: Some(&dir),
                command: text(spec, "command"),
                ..Facts::default()
            },
        };
        let kept = match kind {
            Kind::Web => crate::ledger::Kind::Page,
            Kind::Term | Kind::Board => crate::ledger::Kind::Terminal,
        };
        self.own(&key, call, kept, facts);
        if let (Kind::Web, Some(browser)) = (kind, &self.browser) {
            browser.group(&key, &crate::projects::key(call.project.id));
        }
        let tab = parse(&key);
        if let Some(told) =
            (tab.filter(|tab| tab.kind == Kind::Web)).and_then(|tab| self.told(tab.id, mark))
        {
            out.report = format!(
                "{}

{told}",
                out.report
            );
        }
        Ok((format!("{key} ({})", kind.name()), out))
    }
}
