//! Gemini CLI and OpenCode in the app's terminals: each reads one more settings file named by a
//! variable of its own, a layer merged with the person's by server name, so their own servers
//! stay, and that file names the app's MCP server by the bare command (`llxterm mcp`), which is
//! on the PATH of the app's terminals alone. A variable the person set is left as it is.
//!
//! Gemini CLI's system defaults (`GEMINI_CLI_SYSTEM_DEFAULTS_PATH`, from 0.3.0) rank under the
//! person's and the workspace's settings; the variable moves the machine's own defaults file,
//! so it is not set where that file is there. OpenCode's `OPENCODE_CONFIG` ranks over the
//! person's global config and under the project's.

use std::path::Path;

/// An agent's variable and the file it names: where it is kept and what it holds.
struct Layer {
    variable: &'static str,
    file: &'static str,
    contents: &'static str,
    /// A file of the machine's that the variable would hide: none is set over it.
    hides: Option<&'static str>,
}

const LAYERS: [Layer; 2] = [
    Layer {
        variable: "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
        file: "gemini-cli/system-defaults.json",
        contents: r#"{"mcpServers": {"llxterm": {"command": "llxterm", "args": ["mcp"]}}}
"#,
        hides: Some("/Library/Application Support/GeminiCli/system-defaults.json"),
    },
    Layer {
        variable: "OPENCODE_CONFIG",
        file: "opencode/opencode.json",
        contents: r#"{"mcp": {"llxterm": {"type": "local", "command": ["llxterm", "mcp"], "enabled": true}}}
"#,
        hides: None,
    },
];

/// The variables for a terminal whose environment `var` reads, each with the app's file
/// written under `data`.
pub(super) fn layers(var: &dyn Fn(&str) -> Option<String>, data: &Path) -> Vec<(String, String)> {
    let free = |layer: &&Layer| {
        var(layer.variable).is_none_or(|set| set.is_empty())
            && layer.hides.is_none_or(|file| !Path::new(file).exists())
    };
    (LAYERS.iter().filter(free))
        .filter_map(|layer| {
            let file = super::written(&data.join(layer.file), layer.contents)?;
            Some((layer.variable.to_string(), file.to_str()?.to_string()))
        })
        .collect()
}

#[cfg(test)]
#[path = "layers_tests.rs"]
mod tests;
