//! How a server the registry lists is reached, in the catalogue's words. An address it is
//! called at comes first, since it runs no code on the person's machine: streamable HTTP
//! before the older server-sent events, never one with a part left to fill in. Else a package
//! started over its standard streams: an npm one by `npx`, a PyPI one by `uvx`, an image by
//! `docker`. A server with neither is not listed.

use serde_json::Value;

use super::super::text_at;
use crate::catalog::model::{Need, Runs, Server};

/// The way to reach `server` (its `server.json`), and what it needs from the person.
pub(super) fn reached(server: &Value) -> Option<Server> {
    remote(server).or_else(|| package(server))
}

/// The items of the list at `key`.
fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    (value.get(key).and_then(Value::as_array))
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// Whether `text` is all given: no `{part}` left to fill in.
fn literal(text: &str) -> bool {
    !text.contains(['{', '}'])
}

/// A value the person gives, as an entry of `environmentVariables` or `headers` says it.
fn need(entry: &Value, header: bool) -> Option<Need> {
    let name = text_at(entry, "name");
    let flag = |key: &str| entry.get(key).and_then(Value::as_bool).unwrap_or(false);
    let default = Some(text_at(entry, "default"))
        .filter(|given| !given.is_empty())
        .unwrap_or_else(|| text_at(entry, "value"));
    (!name.is_empty()).then(|| Need {
        about: text_at(entry, "description"),
        required: flag("isRequired"),
        secret: flag("isSecret"),
        header,
        default: if literal(&default) {
            default
        } else {
            String::new()
        },
        name,
    })
}

/// The address the server is called at, the streamable kind first.
fn remote(server: &Value) -> Option<Server> {
    let remotes = items(server, "remotes");
    let usable = |kind: &str| {
        remotes.iter().find(|remote| {
            let url = text_at(remote, "url");
            text_at(remote, "type") == kind && url.starts_with("https://") && literal(&url)
        })
    };
    let (remote, sse) = (usable("streamable-http").map(|remote| (remote, false)))
        .or_else(|| usable("sse").map(|remote| (remote, true)))?;
    Some(Server {
        runs: Runs::Remote {
            url: text_at(remote, "url"),
            sse,
        },
        needs: (items(remote, "headers").iter())
            .filter_map(|header| need(header, true))
            .collect(),
    })
}

/// The first package started over its standard streams by a program the agents know.
fn package(server: &Value) -> Option<Server> {
    items(server, "packages").iter().find_map(|package| {
        // Written as `{"type": "stdio"}`, or by an older schema as the word alone.
        let transport = package.get("transport").map(|transport| {
            (transport.as_str().map(str::to_string)).unwrap_or_else(|| text_at(transport, "type"))
        });
        if transport.is_some_and(|kind| kind != "stdio") {
            return None;
        }
        let needs: Vec<Need> = (items(package, "environmentVariables").iter())
            .filter_map(|variable| need(variable, false))
            .collect();
        let (command, mut args) = started(package, &needs)?;
        args.extend(arguments(package, "packageArguments")?);
        Some(Server {
            runs: Runs::Command { command, args },
            needs,
        })
    })
}

/// The program that starts `package` and its arguments up to the package's own.
fn started(package: &Value, needs: &[Need]) -> Option<(String, Vec<String>)> {
    let identifier = text_at(package, "identifier");
    let version = text_at(package, "version");
    if identifier.is_empty() || identifier.starts_with('-') || !literal(&identifier) {
        return None;
    }
    let pinned = |by: &str| match version.is_empty() || version == "latest" {
        true => identifier.clone(),
        false => format!("{identifier}{by}{version}"),
    };
    let runtime = arguments(package, "runtimeArguments")?;
    let (command, mut args) = match text_at(package, "registryType").as_str() {
        "npm" => ("npx", vec!["-y".to_string()]),
        "pypi" => ("uvx", vec![]),
        "oci" => {
            let mut args = vec!["run".to_string(), "-i".into(), "--rm".into()];
            for need in needs {
                args.extend(["-e".to_string(), need.name.clone()]);
            }
            ("docker", args)
        }
        _ => return None,
    };
    args.extend(runtime);
    args.push(match command {
        "npx" => pinned("@"),
        "uvx" => pinned("=="),
        // An image named with its tag already keeps it.
        _ if identifier
            .rsplit('/')
            .next()
            .is_some_and(|last| last.contains(':')) =>
        {
            identifier
        }
        _ => pinned(":"),
    });
    Some((command.to_string(), args))
}

/// The arguments at `key` of `package` that are given whole; none at all when one that is
/// required is not.
fn arguments(package: &Value, key: &str) -> Option<Vec<String>> {
    let mut args = vec![];
    for argument in items(package, key) {
        let value = Some(text_at(argument, "value"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| text_at(argument, "default"));
        let required = argument.get("isRequired").and_then(Value::as_bool) == Some(true);
        let named = text_at(argument, "type") == "named";
        let name = text_at(argument, "name");
        let given = literal(&value) && !(value.is_empty() && !named);
        match (given, named) {
            (true, true) if !name.is_empty() && literal(&name) => {
                args.push(name);
                args.extend(Some(value).filter(|value| !value.is_empty()));
            }
            (true, false) => args.push(value),
            (true | false, true | false) if required => return None,
            (true | false, true | false) => {}
        }
    }
    Some(args)
}

#[cfg(test)]
#[path = "runs_tests.rs"]
mod tests;
