//! The commands that call the app: `window` (one call, `--end`, `--said`, `--said --follow
//! [--json]`) and `status`/`quit`. The CLI is a client of the protocol like any other: its call carries an
//! idempotency key, so one asked again after a broken connection is not done twice, and a
//! picture comes as bytes that it saves where the calling agent can read them.

use std::io::Read;

use serde_json::{Value, json};

use super::{FAILED, OK, UNREACHABLE};
use crate::client::{Answer, Caller, Start, Trouble, ask, window_params};
use crate::out::{say, warn};

/// What `window` was given.
#[derive(Default)]
struct Window {
    json: Option<String>,
    /// `--as NAME`: the agent's name within the project.
    agent: Option<String>,
    /// `--project DIR`: the project, when the caller knows it better than its directory does.
    project: Option<std::path::PathBuf>,
    mode: Mode,
    /// `--said --follow`: keep waiting for notes; `--json` prints each as a JSON string.
    follow: bool,
    json_lines: bool,
    /// `--peek`: a look for a pane; the person's notes stay for the agent's own call.
    peek: bool,
}

/// What `window` does besides a call: none of these starts the app.
#[derive(Default, Clone, Copy, PartialEq)]
enum Mode {
    /// A call (the JSON), or with none, how to call it.
    #[default]
    Call,
    /// `--end`: close the owner's tabs.
    End,
    /// `--said`: what the person sent from the window since last asked.
    Said,
    /// `--board`: the owner's open cards, read from the board's file.
    Board,
}

fn parse(args: &[String]) -> Result<Window, String> {
    let mut parsed = Window::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--end" | "--said" | "--board" if parsed.mode != Mode::Call => {
                return Err("one of --end, --said and --board only".into());
            }
            "--end" => parsed.mode = Mode::End,
            "--said" => parsed.mode = Mode::Said,
            "--board" => parsed.mode = Mode::Board,
            "--follow" => parsed.follow = true,
            // As before, `--said --json` follows: `--json` is a way of following.
            "--json" => {
                parsed.json_lines = true;
                parsed.follow = true;
            }
            "--peek" => parsed.peek = true,
            "--as" => {
                let name = value(args.next()).ok_or("--as needs a name")?;
                parsed.agent = Some(name.clone());
            }
            "--project" => {
                let dir = value(args.next()).ok_or("--project needs a folder")?;
                parsed.project = Some(std::path::PathBuf::from(dir));
            }
            flag if flag.starts_with("--") => return Err(format!("no option `{flag}`")),
            _ if parsed.json.is_some() => return Err("one JSON argument only".into()),
            json => parsed.json = Some(json.to_string()),
        }
    }
    // What applies to nothing is refused, not dropped: `--end '{...}'` would close every tab.
    if (parsed.follow || parsed.json_lines) && parsed.mode != Mode::Said {
        return Err("--follow and --json go with --said".into());
    }
    if parsed.mode != Mode::Call && parsed.json.is_some() {
        return Err("--end, --said and --board take no JSON".into());
    }
    Ok(parsed)
}

/// What follows `--as` or `--project`: the value, not the next option.
fn value(next: Option<&String>) -> Option<&String> {
    next.filter(|value| !value.starts_with("--"))
}

/// `--board`: `project`'s board in brief, read from its file, so it needs no app running and starts none.
/// Nothing when it has no open card, or no file (a name given with `--as` has none).
fn board(project: &std::path::Path) -> u8 {
    match crate::board::store::written(project) {
        Ok(board) => {
            if let Some(brief) = board.and_then(|board| board.brief(crate::board::now())) {
                say(&brief);
            }
            OK
        }
        Err(why) => {
            warn(&format!("llxterm: {why}"));
            FAILED
        }
    }
}

pub(super) fn window(args: &[String]) -> u8 {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(why) => return usage(&why),
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let caller = match &parsed.project {
        Some(dir) => Caller::at(&cwd.join(dir), parsed.agent.clone()),
        None => Caller::of(&cwd, parsed.agent.clone()),
    };
    // None of these starts the app: with none there is nothing to end and nothing said, and the
    // board is read from its file.
    let method = match parsed.mode {
        Mode::Call => None,
        Mode::Board => return board(&caller.project),
        Mode::Said if parsed.follow => return crate::follow::follow(&caller, parsed.json_lines),
        Mode::Said => Some("window/said"),
        Mode::End => Some("window/end"),
    };
    if let Some(method) = method {
        return printed(ask(Start::Never, method, &caller.with(json!({}))));
    }
    let Some(json) = parsed.json else {
        say(&crate::desk::description());
        say(&format!(
            "\nArguments (JSON Schema):\n{}",
            serde_json::to_string_pretty(&crate::desk::schema()).unwrap_or_default()
        ));
        return OK;
    };
    let text = if json == "-" { read_stdin() } else { Ok(json) };
    let args = match text.and_then(|text| object(&text)) {
        Ok(args) => args,
        Err(why) => return usage(&why),
    };
    let mut params = window_params(&caller, &cwd.display().to_string(), &args);
    if parsed.peek {
        params["peek"] = json!(true);
    }
    printed(ask(Start::Spawn, "window/call", &params))
}

fn object(text: &str) -> Result<Value, String> {
    const WHAT: &str = "the call is the window's arguments as one JSON object, such as '{\"action\": \"list\"}'; \
                        `llxterm window` alone says what they are";
    match serde_json::from_str(text) {
        Ok(args @ Value::Object(_)) => Ok(args),
        Ok(_) => Err(WHAT.into()),
        Err(why) => Err(format!("{WHAT} ({why})")),
    }
}

/// The most the call may be, read from stdin.
const MOST_STDIN: u64 = 1 << 20;

fn read_stdin() -> Result<String, String> {
    read_up_to(std::io::stdin(), MOST_STDIN)
}

/// All of `input` as text when it is at most `most` bytes; more is refused, not cut (a cut call
/// would fail as bad JSON, and say nothing of why).
fn read_up_to(input: impl Read, most: u64) -> Result<String, String> {
    let mut text = String::new();
    input
        .take(most + 1)
        .read_to_string(&mut text)
        .map_err(|e| format!("stdin could not be read ({e})"))?;
    match u64::try_from(text.len()).is_ok_and(|read| read > most) {
        true => Err(format!(
            "stdin is more than {} MiB: the call is too large",
            most >> 20
        )),
        false => Ok(text),
    }
}

/// `method` asked of the running app, never starting it (`status`, `quit`); with none running,
/// it says so and ends with `idle`.
pub(super) fn admin(method: &str, idle: u8) -> u8 {
    let asked = ask(Start::Never, method, &json!({}));
    if let Err(Trouble::None) = &asked {
        say("llxterm: Localix Terminal is not running");
        return idle;
    }
    printed(asked)
}

/// The answer printed: the text on stdout when it came to what was asked, else on stderr.
fn printed(answer: Result<Answer, Trouble>) -> u8 {
    match answer {
        Ok(answer) => {
            let words = words(&answer, &crate::paths::shots());
            if !words.is_empty() {
                say(&words);
            }
            OK
        }
        Err(Trouble::None) => OK,
        Err(Trouble::Refused(why)) => {
            warn(&why);
            FAILED
        }
        Err(Trouble::Lost(why)) => usage(&why),
    }
}

/// What an answer says: its text, and for a picture that came as bytes the file it was saved
/// to in `shots`, where the calling agent reads it.
fn words(answer: &Answer, shots: &std::path::Path) -> String {
    let text = answer.value["text"].as_str().unwrap_or_default();
    let picture = answer.blob.as_ref().map(|blob| {
        let (width, height) = (
            &answer.value["shot"]["width"],
            &answer.value["shot"]["height"],
        );
        crate::shots::save(shots, &blob.bytes).map_or_else(
            |why| format!("The screenshot could not be saved: {why}"),
            |path| {
                format!(
                    "Screenshot saved ({width}x{height}): {}\nRead that file to see it.",
                    path.display()
                )
            },
        )
    });
    let parts = [Some(text), picture.as_deref()];
    let kept: Vec<&str> = parts
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect();
    kept.join("\n\n")
}

pub(super) fn usage(why: &str) -> u8 {
    warn(&format!("llxterm: {why}"));
    UNREACHABLE
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
