//! Where a listed thing itself lives, as one name whichever source lists it: two marketplaces
//! that list one plugin give it the same origin, so the index shows it once. A repository is
//! `host/owner/repo`, lowercase, with no scheme, user or `.git`; a branch or tag it is taken
//! at follows as `@ref` (but the default branches, which name no other version, and a commit,
//! which every new revision changes); a folder in it as `#path`.

use serde_json::Value;

use super::text_at;

/// The branches that say no more than "the repository as it is".
const DEFAULT_BRANCHES: [&str; 3] = ["main", "master", "HEAD"];

/// A repository's address as an origin: `github.com/owner/repo`; `owner/repo` alone is on
/// GitHub. Empty when it is no address.
pub(super) fn address(url: &str) -> String {
    let url = url.trim();
    let bare = url.split_once("://").map_or(url, |(_, rest)| rest);
    let bare = bare.split_once('@').map_or(bare, |(user, rest)| {
        // A user before the host (`git@github.com:o/r`); an `@` later is part of the path.
        match user.contains('/') {
            true => bare,
            false => rest,
        }
    });
    let bare = match url.contains("://") {
        true => bare.to_string(),
        // `host:owner/repo`, as ssh writes it.
        false => bare.replacen(':', "/", 1),
    };
    let bare = bare.trim_end_matches('/');
    let bare = bare
        .strip_suffix(".git")
        .unwrap_or(bare)
        .trim_end_matches('/');
    let parts: Vec<&str> = bare.split('/').collect();
    let plain = |part: &&str| {
        !part.is_empty()
            && (part.chars()).all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    if parts.len() < 2 || !parts.iter().all(plain) {
        return String::new();
    }
    let hosted = parts.first().is_some_and(|host| host.contains('.'));
    match (hosted, parts.len()) {
        (true, 3..) => bare.to_lowercase(),
        (false, 2) => format!("github.com/{}", bare.to_lowercase()),
        (true | false, _) => String::new(),
    }
}

/// A folder as an origin writes it: no `./`, no slash at either end; none when it climbs out.
fn folder(path: &str) -> Option<String> {
    let path = path.trim();
    let path = path.strip_prefix("./").unwrap_or(path).trim_matches('/');
    (!path.split('/').any(|part| part == "..")).then(|| path.to_string())
}

/// `base` with `rev` and the folder `path`, as an origin.
fn joined(base: String, rev: &str, path: &str) -> String {
    if base.is_empty() {
        return base;
    }
    let Some(path) = folder(path) else {
        return String::new();
    };
    let commit = rev.len() >= 7 && rev.chars().all(|c| c.is_ascii_hexdigit());
    let mut origin = base;
    if !rev.is_empty() && !commit && !DEFAULT_BRANCHES.contains(&rev) {
        origin.push('@');
        origin.push_str(rev);
    }
    if !path.is_empty() {
        origin.push('#');
        origin.push_str(&path);
    }
    origin
}

/// The origin of a plugin a marketplace kept in `home` (`owner/repo`) lists, from its
/// `source`: a folder of the marketplace's own repository, or a repository named.
pub(super) fn of_plugin(home: &str, source: &Value) -> String {
    let Value::Object(given) = source else {
        return match source.as_str() {
            Some(path) => joined(address(home), "", path),
            None => String::new(),
        };
    };
    let kind = given.get("source").and_then(Value::as_str).unwrap_or("");
    let base = match kind {
        "local" => address(home),
        "url" | "git-subdir" => address(&text_at(source, "url")),
        "github" => address(&text_at(source, "repo")),
        _ => return String::new(),
    };
    joined(base, &text_at(source, "ref"), &text_at(source, "path"))
}

/// The origin of a skill in the folder `path` of the GitHub repository `repo`.
pub(super) fn of_skill(repo: &str, path: &str) -> String {
    joined(address(repo), "", path)
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod tests;
