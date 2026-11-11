//! Tests of `place`: the configuration file each agent itself reads.

use std::path::{Path, PathBuf};

use super::{Place, shown};
use crate::setup::mcp_hosts::host;
use crate::testing::Scratch;

/// Where agent `id` keeps its configuration, with the variables `set`.
fn config(id: &str, home: &Path, set: &[(&str, &str)]) -> PathBuf {
    let var = |name: &str| {
        (set.iter())
            .find(|(known, _)| *known == name)
            .map(|(_, value)| (*value).to_string())
    };
    host(id).unwrap().place.config(home, var)
}

#[test]
fn every_agent_keeps_its_configuration_under_the_home_directory_until_it_is_moved() {
    let home = Path::new("/Users/me");
    let under = |id: &str| config(id, home, &[]).display().to_string();
    assert_eq!(under("codex"), "/Users/me/.codex/config.toml");
    assert_eq!(under("gemini"), "/Users/me/.gemini/settings.json");
    assert_eq!(under("copilot"), "/Users/me/.copilot/mcp-config.json");
    assert_eq!(
        under("opencode"),
        "/Users/me/.config/opencode/opencode.json"
    );
    assert_eq!(under("hermes"), "/Users/me/.hermes/config.yaml");
    // Each agent's own variable names the folder in place of its usual one.
    let moved = |id: &str, var: &str| config(id, home, &[(var, "/work/x")]).display().to_string();
    assert_eq!(moved("codex", "CODEX_HOME"), "/work/x/config.toml");
    assert_eq!(
        moved("gemini", "GEMINI_CLI_HOME"),
        "/work/x/.gemini/settings.json"
    );
    assert_eq!(moved("copilot", "COPILOT_HOME"), "/work/x/mcp-config.json");
    assert_eq!(
        moved("opencode", "XDG_CONFIG_HOME"),
        "/work/x/opencode/opencode.json"
    );
    assert_eq!(moved("hermes", "HERMES_HOME"), "/work/x/config.yaml");
    // Another agent's variable, or one set to nothing, moves none.
    assert_eq!(moved("codex", "HERMES_HOME"), under("codex"));
    assert_eq!(
        config("codex", home, &[("CODEX_HOME", " ")]),
        config("codex", home, &[])
    );
}

#[test]
fn hermes_reads_the_profile_in_use() {
    let scratch = Scratch::new();
    let home = scratch.path();
    let usual = home.join(".hermes/config.yaml");
    assert_eq!(config("hermes", home, &[]), usual);
    scratch.file(".hermes/active_profile", "default\n");
    assert_eq!(config("hermes", home, &[]), usual);
    scratch.file(".hermes/active_profile", "\u{feff}work\n");
    assert_eq!(
        config("hermes", home, &[]),
        home.join(".hermes/profiles/work/config.yaml")
    );
    // A name that is no one folder's is not followed out of the profiles.
    scratch.file(".hermes/active_profile", "../../elsewhere");
    assert_eq!(config("hermes", home, &[]), usual);
    // A home that is a profile's own folder already is used as it is.
    let root = scratch.file("moved/profiles/night/active_profile", "work");
    let night = root.parent().unwrap();
    assert_eq!(
        config(
            "hermes",
            home,
            &[("HERMES_HOME", &night.display().to_string())]
        ),
        night.join("config.yaml")
    );
    // Only an agent that keeps profiles looks for one.
    let plain = Place {
        dir: ".hermes",
        var: "X",
        file: "config.yaml",
        profiles: false,
    };
    scratch.file(".hermes/active_profile", "work");
    assert_eq!(plain.config(home, |_| None), usual);
}

#[test]
fn a_file_is_shown_from_the_home_directory_when_it_is_under_it() {
    let home = Path::new("/Users/me");
    assert_eq!(
        shown(Path::new("/Users/me/.codex/config.toml"), home),
        "~/.codex/config.toml"
    );
    assert_eq!(
        shown(Path::new("/work/x/config.toml"), home),
        "/work/x/config.toml"
    );
}
