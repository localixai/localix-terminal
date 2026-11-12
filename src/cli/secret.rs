//! `llxterm secret`: a secret saved from stdin under a name (`secret NAME < FILE`), the
//! names saved (`secret`), or one forgotten (`secret --forget NAME`). No command prints a value.

use crate::out::{say, warn};

use super::call::usage;
use super::{FAILED, OK};

pub(super) fn run(args: &[String]) -> u8 {
    let Some(dir) = crate::secrets::dir() else {
        return usage("no config directory: set HOME or XDG_CONFIG_HOME");
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => {
            let names = crate::secrets::names_in(&dir);
            if names.is_empty() {
                say("No secrets saved: `llxterm secret NAME < FILE` saves one.");
            } else {
                let refs: Vec<String> =
                    names.iter().map(|n| crate::secrets::reference(n)).collect();
                say(&refs.join("\n"));
            }
            OK
        }
        ["--forget", name] => match crate::secrets::forget_in(&dir, name) {
            Ok(true) => {
                say(&format!("Secret {name} forgotten."));
                OK
            }
            Ok(false) => usage(&format!("no secret {name}")),
            Err(why) => {
                warn(&format!("llxterm: cannot forget {name}: {why}"));
                FAILED
            }
        },
        [name] if !name.starts_with('-') => save(&dir, name),
        _ => {
            usage("secret takes NAME (the value on stdin), --forget NAME, or nothing to list them")
        }
    }
}

fn save(dir: &std::path::Path, name: &str) -> u8 {
    // On a terminal there is no prompt, and what is typed would show (and stay in its scrollback).
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return usage(&format!(
            "give the value on stdin, not typed: llxterm secret {name} < FILE"
        ));
    }
    let mut value = String::new();
    if let Err(why) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut value) {
        return usage(&format!("cannot read the secret from stdin: {why}"));
    }
    match crate::secrets::save_in(dir, name, &value) {
        Ok(path) => {
            say(&format!(
                "Secret {name} is saved in {}. A call names it as {}: in a page's `fill`, or a terminal's input.",
                path.display(),
                crate::secrets::reference(name)
            ));
            OK
        }
        // A name or a value that is no good is the arguments'; the rest is the disk's.
        Err(why) if why.kind() == std::io::ErrorKind::InvalidInput => {
            usage(&format!("cannot save secret {name}: {why}"))
        }
        Err(why) => {
            warn(&format!("llxterm: cannot save secret {name}: {why}"));
            FAILED
        }
    }
}
