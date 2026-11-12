//! Tests of `catalog::source::marketplace`, against entries of the real files: Claude Code's
//! official directory, its community directory and the Codex directory.

use super::*;
use crate::catalog::source::tests::{Fake, new};

const OFFICIAL: Marketplace = Marketplace {
    id: "claude-plugins",
    title: "Claude Plugins",
    repo: "anthropics/claude-plugins-official",
    trust: 3,
    format: PluginFormat::Claude,
};

const COMMUNITY: Marketplace = Marketplace {
    id: "claude-community",
    title: "Claude Community",
    repo: "anthropics/claude-plugins-community",
    trust: 2,
    format: PluginFormat::Claude,
};

const CODEX: Marketplace = Marketplace {
    id: "codex-plugins",
    title: "Codex Plugins",
    repo: "openai/plugins",
    trust: 3,
    format: PluginFormat::Codex,
};

/// The official directory's file, a few of its entries kept and their descriptions cut short.
const FILE: &str = r#"{"$schema":"https://anthropic.com/claude-code/marketplace.schema.json",
"name":"claude-plugins-official","owner":{"name":"Anthropic","email":"support@anthropic.com"},
"plugins":[
{"name":"42crunch-api-security-testing","description":"Automate API security directly in Claude Code with 42Crunch","author":{"name":"42Crunch"},"category":"security","source":{"source":"git-subdir","url":"https://github.com/42Crunch-AI/claude-plugins.git","path":"plugins/api-security-testing","ref":"v1.5.5","sha":"faf5305385de8afed9468904e8639be737aff39e"},"homepage":"https://42crunch.com"},
{"name":"agent-sdk-dev","description":"Development kit for working with the Claude Agent SDK","author":{"name":"Anthropic","email":"support@anthropic.com"},"source":"./plugins/agent-sdk-dev","category":"development","homepage":"https://github.com/anthropics/claude-plugins-public/tree/main/plugins/agent-sdk-dev"},
{"name":"agentforce-adlc","description":"Agentforce Agent Development Life Cycle","category":"development","source":{"source":"url","url":"https://github.com/SalesforceAIResearch/agentforce-adlc.git","sha":"09bf1539d41f9ff355ba3eb5d05d4a75813423bb"},"homepage":"https://github.com/SalesforceAIResearch/agentforce-adlc"},
{"name":"clangd-lsp","description":"C/C++ language server (clangd) for code intelligence","version":"1.0.0","author":{"name":"Anthropic","email":"support@anthropic.com"},"source":"./plugins/clangd-lsp","category":"development","strict":false,"lspServers":{"clangd":{"command":"clangd","args":["--background-index"]}}},
{"name":"xcode","displayName":"Xcode","description":"Work with Xcode projects from Claude Code","author":{"name":"Apple Inc."},"category":"development","source":{"source":"command","command":"xcrun agent plugin path --plugin-format claude","mode":"link"},"homepage":"https://developer.apple.com/xcode/"},
{"name":"airwallex-agentos","displayName":"Airwallex","description":"Bring Airwallex's global financial infrastructure to Claude","author":{"name":"Airwallex"},"category":"productivity","source":{"source":"git-subdir","url":"https://github.com/airwallex/airwallex-marketplace.git","path":"plugins/airwallex-agentos","ref":"master","sha":"877551077786664d1cbeb32013443291c66e69e6"},"homepage":"https://www.airwallex.com/docs"},
{"description":"An entry with no name"}
]}"#;

/// The community directory's file: each shape its entries come in, descriptions cut short.
const COMMUNITY_FILE: &str = r#"{"name":"claude-community","owner":{"name":"Anthropic"},"renames":{"twilio":"twilio-developer-kit"},"plugins":[
{"name":"0x","description":"Guide developers through swapping ERC-20 tokens using the 0x API.","source":{"source":"url","url":"https://github.com/0xProject/0x-ai.git","sha":"0167bbb411cc972b966127d23c23de801061fa99"},"homepage":"https://github.com/0xProject/0x-ai"},
{"name":"cowork-plugin-management","source":"./cowork-plugin-management","description":"Create, customize, and manage plugins tailored to your organization's tools."},
{"name":"42crunch-api-security-testing","description":"Automate API security directly in Claude Code with 42Crunch.","source":{"source":"git-subdir","url":"42Crunch-AI/claude-plugins","path":"plugins/api-security-testing","ref":"v1.0.1","sha":"30287f5e3f122a646d1ac5ca3ab96e130c52a3ad"},"homepage":"https://docs.42crunch.com"},
{"name":"meticulous","description":"Agent skills for Meticulous visual regression testing.","author":{"name":"Meticulous"},"category":"testing","source":{"source":"url","url":"https://github.com/alwaysmeticulous/skills.git","sha":"59788561ed2b53f0fc56c54048c09f47f35b61af"},"homepage":"https://www.meticulous.ai/"}
]}"#;

/// The Codex directory's file, a few of its entries kept, and one it does not make available.
const CODEX_FILE: &str = r#"{"name":"openai-curated","interface":{"displayName":"Codex official"},"plugins":[
{"name":"linear","source":{"source":"local","path":"./plugins/linear"},"policy":{"installation":"AVAILABLE","authentication":"ON_INSTALL"},"category":"Productivity"},
{"name":"google-drive","source":{"source":"local","path":"./plugins/google-drive"},"policy":{"installation":"AVAILABLE","authentication":"ON_INSTALL"},"category":"Productivity"},
{"name":"crowdstrike-falcon-foundry","source":{"source":"url","url":"https://github.com/CrowdStrike/foundry-skills.git","sha":"0123456789abcdef0123"},"policy":{"installation":"AVAILABLE","authentication":"ON_INSTALL"},"category":"Developer Tools","interface":{"displayName":"CrowdStrike Falcon Foundry"}},
{"name":"withdrawn","source":{"source":"local","path":"./plugins/withdrawn"},"policy":{"installation":"NOT_AVAILABLE"},"category":"Productivity"}
]}"#;

fn ids(listings: &[Listing]) -> Vec<&str> {
    listings.iter().map(|listing| listing.id.as_str()).collect()
}

#[test]
fn every_plugin_but_one_a_command_installs_is_listed_as_the_directory_says() {
    let listings = OFFICIAL.listings(FILE).unwrap();
    assert_eq!(
        ids(&listings),
        [
            "claude-plugins/42crunch-api-security-testing",
            "claude-plugins/agent-sdk-dev",
            "claude-plugins/agentforce-adlc",
            "claude-plugins/clangd-lsp",
            "claude-plugins/airwallex-agentos",
        ]
    );
    let crunch = &listings[0];
    assert_eq!(crunch.what, What::Plugin);
    assert_eq!(crunch.source, "claude-plugins");
    assert_eq!(crunch.name, "42crunch-api-security-testing");
    assert_eq!(crunch.title, "42crunch-api-security-testing");
    assert_eq!(crunch.author, "42Crunch");
    assert_eq!(crunch.category, "security");
    assert_eq!(crunch.homepage, "https://42crunch.com");
    // No version of its own: its commit's.
    assert_eq!(crunch.version, "faf5305385de");
    assert_eq!(
        crunch.origin,
        "github.com/42crunch-ai/claude-plugins@v1.5.5#plugins/api-security-testing"
    );
    assert_eq!(
        crunch.recipe,
        Recipe::Plugin {
            marketplace: "claude-plugins-official".into(),
            from: "anthropics/claude-plugins-official".into(),
            format: PluginFormat::Claude,
        }
    );
    // A version of its own wins; a folder of the marketplace pins none.
    assert_eq!(listings[3].version, "1.0.0");
    assert_eq!(listings[1].version, "");
    assert_eq!(
        listings[3].origin,
        "github.com/anthropics/claude-plugins-official#plugins/clangd-lsp"
    );
    // A folder with no page of its own is shown where it is kept.
    assert_eq!(
        listings[3].homepage,
        "https://github.com/anthropics/claude-plugins-official/tree/HEAD/plugins/clangd-lsp"
    );
    // A name to show is the title; the default branch is no part of the origin.
    assert_eq!(listings[4].title, "Airwallex");
    assert_eq!(
        listings[4].origin,
        "github.com/airwallex/airwallex-marketplace#plugins/airwallex-agentos"
    );
    assert_eq!(listings[2].author, "");
    assert_eq!(
        listings[2].origin,
        "github.com/salesforceairesearch/agentforce-adlc"
    );
}

#[test]
fn the_community_directory_is_read_as_the_official_one_is_and_each_shape_has_its_origin() {
    let listings = COMMUNITY.listings(COMMUNITY_FILE).unwrap();
    assert_eq!(listings.len(), 4);
    let origins: Vec<&str> = (listings.iter())
        .map(|listing| listing.origin.as_str())
        .collect();
    assert_eq!(
        origins,
        [
            "github.com/0xproject/0x-ai",
            "github.com/anthropics/claude-plugins-community#cowork-plugin-management",
            "github.com/42crunch-ai/claude-plugins@v1.0.1#plugins/api-security-testing",
            "github.com/alwaysmeticulous/skills",
        ]
    );
    let zero = &listings[0];
    assert_eq!(zero.id, "claude-community/0x");
    assert_eq!(zero.category, "");
    assert_eq!(zero.version, "0167bbb411cc");
    assert_eq!(
        zero.recipe,
        Recipe::Plugin {
            marketplace: "claude-community".into(),
            from: "anthropics/claude-plugins-community".into(),
            format: PluginFormat::Claude,
        }
    );
    assert_eq!(listings[3].author, "Meticulous");
}

#[test]
fn every_available_codex_plugin_is_listed_by_its_name_made_readable() {
    let listings = CODEX.listings(CODEX_FILE).unwrap();
    let titles: Vec<&str> = (listings.iter())
        .map(|listing| listing.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["Linear", "Google Drive", "CrowdStrike Falcon Foundry"]
    );
    let drive = &listings[1];
    assert_eq!(drive.id, "codex-plugins/google-drive");
    assert_eq!(drive.name, "google-drive");
    assert_eq!(drive.summary, "");
    assert_eq!(drive.category, "Productivity");
    assert_eq!(
        drive.homepage,
        "https://github.com/openai/plugins/tree/HEAD/plugins/google-drive"
    );
    assert_eq!(
        drive.origin,
        "github.com/openai/plugins#plugins/google-drive"
    );
    assert_eq!(drive.version, "");
    assert_eq!(
        drive.recipe,
        Recipe::Plugin {
            marketplace: "openai-curated".into(),
            from: "openai/plugins".into(),
            format: PluginFormat::Codex,
        }
    );
    // One from elsewhere pinned to a commit: that commit, and no page of this repository.
    assert_eq!(listings[2].version, "0123456789ab");
    assert_eq!(listings[2].homepage, "");
    assert_eq!(listings[2].origin, "github.com/crowdstrike/foundry-skills");
}

#[test]
fn a_file_that_is_no_directory_says_why() {
    assert!(OFFICIAL.listings("<html>").is_err());
    assert!(OFFICIAL.listings(r#"{"plugins":[]}"#).is_err());
    assert!(CODEX.listings("[]").is_err());
}

#[test]
fn each_format_is_read_from_its_own_file_of_the_repository() {
    assert_eq!(
        OFFICIAL.url(),
        "https://raw.githubusercontent.com/anthropics/claude-plugins-official/HEAD/.claude-plugin/marketplace.json"
    );
    assert_eq!(
        CODEX.url(),
        "https://raw.githubusercontent.com/openai/plugins/HEAD/.agents/plugins/marketplace.json"
    );
    assert_eq!(
        (COMMUNITY.id(), COMMUNITY.title(), COMMUNITY.trust()),
        ("claude-community", "Claude Community", 2)
    );
}

#[test]
fn the_directory_is_read_again_by_its_etag_and_an_unchanged_one_is_nothing_new() {
    let net = Fake::answering([new(FILE, Some("\"e79\"")), Ok(Fetched::Same)]);
    let cache = std::env::temp_dir();
    let batch = OFFICIAL.read(&net, None, &cache).unwrap().unwrap();
    assert_eq!(batch.listings.len(), 5);
    assert_eq!(batch.mark, "\"e79\"");
    assert!(batch.whole && !batch.more && batch.gone.is_empty());
    assert_eq!(
        OFFICIAL.read(&net, Some(&batch.mark), &cache).unwrap(),
        None
    );
    let asked = net.asked();
    assert_eq!(asked[0], (OFFICIAL.url(), None));
    assert_eq!(asked[1], (OFFICIAL.url(), Some("\"e79\"".to_string())));
}
