# desk for Claude Code

Claude opens real pages and terminals in a window you watch, through the MCP tools of `llx-desk mcp`
(`open`, `act`, `look`, `screenshot`, `tabs`, `comments`), which keep one connection to the window's daemon
for the whole session. The command line, `llx-desk window`, is the fallback.

## Install

1. The plugin, in Claude Code:

   ```
   /plugin marketplace add rsaryev/llx-desk
   /plugin install llx-desk@llx-desk
   ```

2. Nothing else: the plugin's `bin/llx-desk` runs a program of the plugin's version, one on `PATH` if
   it is that version (a build from source, `install.sh`), else the release build for macOS or Linux
   x86_64, downloaded and checked against the release's `SHA256SUMS` the first time and kept in
   `~/.local/share/llx-desk`. On Linux, pages need WebKitGTK 4.1 and GTK 3.
3. Optional: an OpenRouter key, which Claude Code asks for when the plugin is enabled (kept in the system
   keychain; leave it empty to skip, and set it later in `/plugin`). Only `goal` needs it, where Claude hands
   a screen of clicks to a fast helper model; `llx-desk key < FILE` or `OPENROUTER_API_KEY` work too.
4. Allow the tools when Claude asks (`look` and `screenshot` only read), or `Bash(llx-desk:*)` for the command line.

## What it adds

- **MCP server `desk`** (`.mcp.json`, started as `bin/llx-desk mcp`): six tools; a screenshot is an image, a goal's steps are progress.
- **Skill `window`** (the model's, hidden from the `/` menu): when to open pages and terminals and how; the tools first, the command line as the fallback.
- **Commands**: `/tabs` (this project's tabs and your latest comments, in the chat), `/llx-desk:end`
  (close this project's tabs), `/llx-desk:stop` (stop the daemon and its window).
- **A pane** (`hooks/register.tsx`): follows the comments you send from the window
  (`llx-desk window --said --follow --json`) and sends each to Claude as a message.
- **Hook**: `SessionEnd` closes the tabs the project opened (`llx-desk window --end`). It does
  nothing when `llx-desk` is not installed.

Two sessions in one project share its tabs; use `--as NAME` to keep them apart.

## Developing

`claude --plugin-dir plugins/claude-code` runs Claude Code with this plugin;
`claude plugin validate .` checks the marketplace and `claude plugin validate plugins/claude-code` the plugin.
