//! Tests of `catalog::source::origin`: every way a marketplace writes where a plugin lives,
//! read as one name.

use serde_json::json;

use super::*;

const HOME: &str = "anthropics/claude-plugins-official";

#[test]
fn an_address_is_read_as_its_host_and_path_lowercase() {
    for (url, origin) in [
        ("https://github.com/Owner/Repo.git", "github.com/owner/repo"),
        ("https://github.com/owner/repo/", "github.com/owner/repo"),
        (
            "https://github.com/owner/repo.git/",
            "github.com/owner/repo",
        ),
        ("http://github.com/owner/repo", "github.com/owner/repo"),
        ("git@github.com:Owner/Repo.git", "github.com/owner/repo"),
        ("ssh://git@github.com/owner/repo", "github.com/owner/repo"),
        ("Owner/Repo", "github.com/owner/repo"),
        (
            "https://gitlab.com/group/sub/repo.git",
            "gitlab.com/group/sub/repo",
        ),
        ("  ", ""),
        ("not an address", ""),
    ] {
        assert_eq!(address(url), origin, "{url}");
    }
}

#[test]
fn a_source_object_is_its_repository_with_its_ref_and_folder() {
    for (source, origin) in [
        (
            json!({"source": "url", "url": "https://github.com/box/skills.git", "sha": "84320fd8984b95cab430335cd341d8314c14bbcb"}),
            "github.com/box/skills",
        ),
        (
            json!({"source": "git-subdir", "url": "https://github.com/pydantic/skills.git", "path": "plugins/logfire", "ref": "main", "sha": "238d"}),
            "github.com/pydantic/skills#plugins/logfire",
        ),
        (
            json!({"source": "git-subdir", "url": "42Crunch-AI/claude-plugins", "path": "./plugins/api-security-testing/", "ref": "v1.0.1"}),
            "github.com/42crunch-ai/claude-plugins@v1.0.1#plugins/api-security-testing",
        ),
        (
            json!({"source": "url", "url": "https://github.com/a/b", "ref": "master"}),
            "github.com/a/b",
        ),
        (
            json!({"source": "url", "url": "https://github.com/a/b", "ref": "HEAD"}),
            "github.com/a/b",
        ),
        (
            json!({"source": "url", "url": "https://github.com/a/b", "ref": "a01396401d7e9e03ed2ac7737c26f1bb76de5028"}),
            "github.com/a/b",
        ),
        (
            json!({"source": "github", "repo": "Owner/Repo", "ref": "develop"}),
            "github.com/owner/repo@develop",
        ),
        (
            json!({"source": "local", "path": "./plugins/linear"}),
            "github.com/anthropics/claude-plugins-official#plugins/linear",
        ),
        (
            json!("./plugins/agent-sdk-dev"),
            "github.com/anthropics/claude-plugins-official#plugins/agent-sdk-dev",
        ),
        (json!("./"), "github.com/anthropics/claude-plugins-official"),
        (json!("../elsewhere"), ""),
        (json!({"source": "command", "command": "x"}), ""),
        (json!({"source": "npm", "package": "x"}), ""),
        (json!(7), ""),
    ] {
        assert_eq!(of_plugin(HOME, &source), origin, "{source}");
    }
}

#[test]
fn the_same_plugin_listed_by_its_own_marketplace_and_by_another_has_one_origin() {
    let own = of_plugin(HOME, &json!("./plugins/logfire"));
    let elsewhere = of_plugin(
        "anthropics/claude-plugins-community",
        &json!({"source": "git-subdir", "url": "https://github.com/anthropics/claude-plugins-official.git", "path": "plugins/logfire", "ref": "main", "sha": "238d97102650c1caa51f35027aee13c469c59542"}),
    );
    assert_eq!(own, elsewhere);
}

#[test]
fn a_skill_is_its_folder_in_its_repository() {
    assert_eq!(
        of_skill("Anthropics/Skills", "skills/pdf"),
        "github.com/anthropics/skills#skills/pdf"
    );
}
