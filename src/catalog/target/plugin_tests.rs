//! Tests of `plugin`: the command lines each agent's program is run with, read before anything
//! runs, and what those programs answer, judged.

use super::super::tests::{env_at, plugin as listing, target};
use super::{Ran, by_status, claude_outcome, plugin};
use crate::catalog::PluginFormat;
use crate::testing::Scratch;

fn ran(ok: bool, out: &str) -> Ran {
    Ran {
        ok,
        out: out.into(),
    }
}

#[test]
fn claude_codes_answer_is_its_last_json_line() {
    assert_eq!(
        claude_outcome(&ran(true, r#"{"outcome":"ok","message":"Installed"}"#)),
        Ok(())
    );
    let failed = r#"{"outcome":"failed","message":"Plugin \"nope\" not found","failureCode":"not_found"}
✘ Failed to install plugin "nope@acme": Plugin "nope" not found"#;
    assert_eq!(
        claude_outcome(&ran(false, failed)).unwrap_err(),
        r#"Plugin "nope" not found"#
    );
    // Already as asked is done: installed before, or not there to uninstall.
    let there = r#"{"outcome":"failed","message":"already","failureCode":"already_in_goal_state"}"#;
    assert_eq!(claude_outcome(&ran(false, there)), Ok(()));
    let flag = r#"{"outcome":"skipped","alreadyInGoalState":true}"#;
    assert_eq!(claude_outcome(&ran(true, flag)), Ok(()));
    let absent = "{\"outcome\":\"failed\",\"message\":\"not installed\",\"failureCode\":\"not_installed\"}\n✘ Failed";
    assert_eq!(claude_outcome(&ran(false, absent)), Ok(()));
    // Without its JSON, its exit status says.
    assert_eq!(
        claude_outcome(&ran(false, "error: unknown option")).unwrap_err(),
        "error: unknown option"
    );
    assert_eq!(claude_outcome(&ran(true, "")), Ok(()));
}

#[test]
fn a_program_that_answers_by_its_status_alone_is_judged_by_it() {
    assert_eq!(by_status(&ran(true, "whatever")), Ok(()));
    assert_eq!(
        by_status(&ran(false, " Error: no such plugin \n")).unwrap_err(),
        "Error: no such plugin"
    );
    assert_eq!(
        by_status(&ran(false, "")).unwrap_err(),
        "it failed and said nothing"
    );
}

#[test]
fn a_plugin_is_passed_as_plain_names_only() {
    let good = listing("pdf-tools", "acme", PluginFormat::Claude);
    let it = plugin(&good).unwrap();
    assert_eq!(it.id(), "pdf-tools@acme");
    assert_eq!(it.from, "acme/plugins");
    assert!(plugin(&listing("--yes", "acme", PluginFormat::Claude)).is_err());
    assert!(plugin(&listing("pdf", "../x", PluginFormat::Claude)).is_err());
    assert!(plugin(&listing("p d f", "acme", PluginFormat::Claude)).is_err());
    let mut elsewhere = listing("pdf", "acme", PluginFormat::Claude);
    if let crate::catalog::Recipe::Plugin { from, .. } = &mut elsewhere.recipe {
        *from = "https://example.com/x".into();
    }
    let why = plugin(&elsewhere).unwrap_err();
    assert_eq!(
        why,
        "`https://example.com/x` is no GitHub repository (owner/repo)"
    );
}

#[test]
fn claude_codes_plan_adds_a_marketplace_it_does_not_know() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let pdf = listing("pdf", "acme", PluginFormat::Claude);
    let plan = target("claude").plan(&pdf, &env).unwrap();
    assert_eq!(
        plan.steps,
        [
            "Run: claude plugin marketplace add acme/plugins",
            "Run: claude plugin install pdf@acme --scope user"
        ]
    );
    assert_eq!(
        plan.after,
        "Run /reload-plugins in Claude Code, or start a new session"
    );
    scratch.file(
        ".claude/plugins/known_marketplaces.json",
        r#"{"acme": {"source": {"source": "github", "repo": "acme/plugins"}}}"#,
    );
    let plan = target("claude").plan(&pdf, &env).unwrap();
    assert_eq!(
        plan.steps,
        ["Run: claude plugin install pdf@acme --scope user"]
    );
}

#[test]
fn codexs_plan_reads_its_marketplaces_from_its_config() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let pdf = listing("pdf", "acme", PluginFormat::Codex);
    let plan = target("codex").plan(&pdf, &env).unwrap();
    assert_eq!(
        plan.steps,
        [
            "Run: codex plugin marketplace add acme/plugins",
            "Run: codex plugin add pdf@acme"
        ]
    );
    assert!(plan.after.contains("/hooks"), "{}", plan.after);
    scratch.file(".codex/config.toml", "[marketplaces.acme]\nsource_type = \"git\"\nsource = \"https://github.com/acme/plugins\"\n");
    let plan = target("codex").plan(&pdf, &env).unwrap();
    assert_eq!(plan.steps, ["Run: codex plugin add pdf@acme"]);
}

#[test]
fn copilots_plan_knows_its_own_marketplace_and_the_ones_it_was_told_of() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let pdf = listing("pdf", "acme", PluginFormat::Claude);
    let plan = target("copilot").plan(&pdf, &env).unwrap();
    assert_eq!(
        plan.steps,
        [
            "Run: copilot plugin marketplace add acme/plugins",
            "Run: copilot plugin install pdf@acme"
        ]
    );
    let built_in = listing("pdf", "awesome-copilot", PluginFormat::Claude);
    let plan = target("copilot").plan(&built_in, &env).unwrap();
    assert_eq!(
        plan.steps,
        ["Run: copilot plugin install pdf@awesome-copilot"]
    );
    scratch.file(".copilot/settings.json", r#"{"extraKnownMarketplaces": {"acme": {"source": {"source": "github", "repo": "acme/plugins"}}}}"#);
    let plan = target("copilot").plan(&pdf, &env).unwrap();
    assert_eq!(plan.steps, ["Run: copilot plugin install pdf@acme"]);
}

#[test]
fn no_plan_accepts_anything_for_the_person() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    for id in ["claude", "codex", "copilot"] {
        let plan = target(id)
            .plan(&listing("pdf", "acme", PluginFormat::Claude), &env)
            .unwrap();
        for step in &plan.steps {
            assert!(
                !step.contains(" -y") && !step.contains("--yes") && !step.contains("--consent"),
                "{step}"
            );
        }
    }
}

#[test]
fn without_its_program_an_install_says_so_and_writes_nothing() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let pdf = listing("pdf", "acme", PluginFormat::Claude);
    let why = target("claude")
        .install(&pdf, &Default::default(), &env)
        .unwrap_err();
    assert!(
        why.contains("Claude Code is not installed (no `claude` on the PATH)"),
        "{why}"
    );
    assert!(std::fs::read_dir(scratch.path()).unwrap().next().is_none());
}
