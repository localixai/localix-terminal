//! The commands that call the daemon: `window` (one call, `--end`, `--said`, `--said --follow
//! [--json]`)
//! and `status`/`stop`. The CLI is a client of protocol v2 like any other: its call carries an
//! idempotency key, so one asked again after a broken connection is not done twice, and a
//! picture comes as bytes that it saves where the calling agent can read them.

use std::io::Read;
use std::sync::Arc;

use serde_json::{Value, json};

use super::{FAILED, OK, UNREACHABLE};
use crate::client::{
    Answer, Client, Error, Notes, Start, Trouble as Unreached, host_project, legacy, window_params,
};
use crate::out::{say, warn};

/// What `window` was given.
#[derive(Default)]
struct Window {
    json: Option<String>,
    owner: Option<String>,
    end: bool,
    said: bool,
    follow: Follow,
}

/// Whether `--said` keeps waiting, and how it prints each note.
#[derive(Default, Clone, Copy, PartialEq)]
enum Follow {
    #[default]
    No,
    /// `--follow`: each note as a block of text.
    Blocks,
    /// `--follow --json`: each note one JSON string on a line of its own (for a program).
    Json,
}

fn parse(args: &[String]) -> Result<Window, String> {
    let mut parsed = Window::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--end" => parsed.end = true,
            "--said" => parsed.said = true,
            "--follow" if parsed.follow == Follow::No => parsed.follow = Follow::Blocks,
            "--follow" => {}
            "--json" => parsed.follow = Follow::Json,
            "--as" => {
                let name = args.next().ok_or("--as needs a name")?;
                parsed.owner = Some(name.clone());
            }
            flag if flag.starts_with("--") => return Err(format!("no option `{flag}`")),
            _ if parsed.json.is_some() => return Err("one JSON argument only".into()),
            json => parsed.json = Some(json.to_string()),
        }
    }
    Ok(parsed)
}

pub(super) fn window(args: &[String]) -> u8 {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(why) => return usage(&why),
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let owner = parsed.owner.clone().unwrap_or_else(|| {
        crate::desk::project(&host_project().unwrap_or_else(|| cwd.clone()))
            .display()
            .to_string()
    });
    if parsed.said && parsed.follow != Follow::No {
        crate::follow::follow(&owner, parsed.follow == Follow::Json);
    }
    if parsed.end || parsed.said {
        // Neither starts a daemon: with none there is nothing to end and nothing said.
        let method = if parsed.end {
            "window/end"
        } else {
            "window/said"
        };
        return printed(ask(Start::Never, method, &json!({"owner": owner})));
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
    printed(ask(
        Start::Spawn,
        "window/call",
        &window_params(&owner, &cwd.display().to_string(), &args),
    ))
}

fn object(text: &str) -> Result<Value, String> {
    match serde_json::from_str(text) {
        Ok(args @ Value::Object(_)) => Ok(args),
        _ => Err("the call is the window's arguments as one JSON object, such as '{\"action\": \"list\"}'; \
                  `llx-desk window` alone says what they are"
            .into()),
    }
}

fn read_stdin() -> Result<String, String> {
    let mut text = String::new();
    std::io::stdin()
        .take(1 << 20)
        .read_to_string(&mut text)
        .map_err(|e| format!("stdin could not be read ({e})"))?;
    Ok(text)
}

/// `method` asked of the running daemon (never starting one): `status`, `stop`. An older
/// daemon is asked in its own protocol, so that this build can end it.
pub(super) fn admin(method: &str, op: &str) -> u8 {
    let asked = ask(Start::Never, method, &json!({}));
    if let Err(Trouble::Older(_)) = &asked {
        return match legacy(op) {
            Ok(text) => {
                say(&text);
                OK
            }
            Err(why) => unreachable_daemon(&why),
        };
    }
    if let Err(Trouble::None) = &asked {
        say("llx-desk: no daemon is running");
        return OK;
    }
    printed(asked)
}

/// Why a call has no answer to print.
enum Trouble {
    /// There is no daemon, and the call did not ask for one.
    None,
    /// The daemon spoke, and said no.
    Refused(String),
    /// A daemon of an older protocol answers the socket.
    Older(String),
    /// The daemon could not be reached.
    Lost(String),
}

/// `method` asked of the daemon, once more after a connection that broke under it.
fn ask(start: Start, method: &str, params: &Value) -> Result<Answer, Trouble> {
    let notes: Notes = Arc::new(|_, _| {});
    // Every request has a key, so that the one made again is not done twice.
    let mut params = params.clone();
    if let Some(fields) = params.as_object_mut() {
        fields
            .entry("idempotencyKey")
            .or_insert_with(|| json!(crate::token()));
    }
    let mut last = Trouble::None;
    for _ in 0..2 {
        let client = match Client::connect(start, &notes) {
            Ok(Some(client)) => client,
            Ok(None) => return Err(Trouble::None),
            Err(Unreached::Older(why)) => return Err(Trouble::Older(why)),
            Err(Unreached::Failed(why)) => return Err(Trouble::Lost(why)),
        };
        match client.request(method, params.clone()) {
            Ok(answer) => return Ok(answer),
            Err(Error::Rpc(error)) => return Err(Trouble::Refused(error.message)),
            Err(other @ (Error::Broken(_) | Error::Timeout | Error::Foreign(_))) => {
                last = Trouble::Lost(other.to_string())
            }
        }
    }
    Err(last)
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
        Err(Trouble::Older(why) | Trouble::Lost(why)) => unreachable_daemon(&why),
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
    warn(&format!("llx-desk: {why}"));
    UNREACHABLE
}

fn unreachable_daemon(why: &str) -> u8 {
    warn(&format!("llx-desk: {why}"));
    UNREACHABLE
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
