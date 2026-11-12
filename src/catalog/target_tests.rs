//! Tests of `target`: which agent takes which listing and is there, and the listings the other
//! targets' tests are given.

use std::path::Path;

use super::{Target, all};
use crate::catalog::{Env, Listing, Need, PluginFormat, Recipe, Runs, Server, What};
use crate::testing::Scratch;

/// A listing of `what`, named `name`, installed by `recipe`.
pub(super) fn listing(what: What, name: &str, recipe: Recipe) -> Listing {
    Listing {
        id: format!("src/{name}"),
        what,
        source: "src".into(),
        name: name.into(),
        title: name.into(),
        summary: String::new(),
        author: String::new(),
        category: String::new(),
        homepage: String::new(),
        version: "1.0.0".into(),
        origin: String::new(),
        recipe,
    }
}

pub(super) fn skill(name: &str) -> Listing {
    let recipe = Recipe::Skill {
        repo: "acme/skills".into(),
        rev: "main".into(),
        path: format!("skills/{name}"),
    };
    listing(What::Skill, name, recipe)
}

pub(super) fn server(name: &str, runs: Runs, needs: Vec<Need>) -> Listing {
    listing(What::Server, name, Recipe::Server(Server { runs, needs }))
}

pub(super) fn command(command: &str, args: &[&str]) -> Runs {
    Runs::Command {
        command: command.into(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
    }
}

pub(super) fn remote(url: &str, sse: bool) -> Runs {
    Runs::Remote {
        url: url.into(),
        sse,
    }
}

/// A need called `name`; `is` says what it is, any of the words `required`, `secret`, `header`.
pub(super) fn need(name: &str, is: &str) -> Need {
    let is = |word: &str| is.split(' ').any(|said| said == word);
    Need {
        name: name.into(),
        about: format!("the {name}"),
        required: is("required"),
        secret: is("secret"),
        header: is("header"),
        default: String::new(),
    }
}

pub(super) fn plugin(name: &str, marketplace: &str, format: PluginFormat) -> Listing {
    let recipe = Recipe::Plugin {
        marketplace: marketplace.into(),
        from: "acme/plugins".into(),
        format,
    };
    listing(What::Plugin, name, recipe)
}

/// The machine of a person whose home is `home`, with nothing on the PATH.
pub(super) fn env_at(home: &Path) -> Env {
    Env {
        home: home.to_path_buf(),
        ..Env::default()
    }
}

/// The target of agent `id`.
pub(super) fn target(id: &str) -> &'static dyn Target {
    *all().iter().find(|target| target.id() == id).unwrap()
}

#[test]
fn every_agent_is_named_as_the_window_and_setup_name_it() {
    let ids: Vec<&str> = all().iter().map(|target| target.id()).collect();
    assert_eq!(ids, ["claude", "codex", "gemini", "copilot", "opencode"]);
    let names: Vec<&str> = all().iter().map(|target| target.name()).collect();
    assert_eq!(
        names,
        [
            "Claude Code",
            "Codex",
            "Gemini CLI",
            "GitHub Copilot",
            "OpenCode"
        ]
    );
}

#[test]
fn an_agent_is_there_when_its_program_is_on_the_given_path_only() {
    let scratch = Scratch::new();
    let bin = scratch.path().join("bin");
    scratch.file("bin/codex", "#!/bin/sh\n");
    let env = Env {
        path: format!("/nowhere:{}", bin.display()),
        ..env_at(scratch.path())
    };
    let there: Vec<&str> = (all().iter())
        .filter(|target| target.present(&env))
        .map(|target| target.id())
        .collect();
    assert_eq!(there, ["codex"]);
    // The process's own PATH is never looked at.
    assert!(
        all()
            .iter()
            .all(|target| !target.present(&env_at(scratch.path())))
    );
}

#[test]
fn each_agent_takes_the_listings_it_can_have() {
    let takes = |listing: &Listing| -> Vec<&str> {
        (all().iter())
            .filter(|target| target.takes(listing))
            .map(|target| target.id())
            .collect()
    };
    let every = ["claude", "codex", "gemini", "copilot", "opencode"];
    assert_eq!(takes(&skill("pdf")), every);
    assert_eq!(takes(&server("w", command("npx", &["w"]), vec![])), every);
    assert_eq!(
        takes(&server("w", remote("https://w.example/mcp", false), vec![])),
        every
    );
    // Codex reaches no server over server-sent events.
    let sse = server("w", remote("https://w.example/sse", true), vec![]);
    assert_eq!(takes(&sse), ["claude", "gemini", "copilot", "opencode"]);
    // A Claude marketplace is read by Claude Code, Codex and Copilot; a Codex one by Codex.
    let claude = plugin("p", "m", PluginFormat::Claude);
    assert_eq!(takes(&claude), ["claude", "codex", "copilot"]);
    assert_eq!(takes(&plugin("p", "m", PluginFormat::Codex)), ["codex"]);
}

#[test]
fn what_an_agent_cannot_have_is_refused_and_never_had() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let gemini = target("gemini");
    let listing = plugin("p", "m", PluginFormat::Claude);
    assert_eq!(
        gemini.plan(&listing, &env).unwrap_err(),
        "Gemini CLI cannot have this plugin"
    );
    assert!(gemini.install(&listing, &Default::default(), &env).is_err());
    assert!(gemini.remove(&listing, &env).is_err());
    assert_eq!(gemini.has(&listing, &env), None);
}
