# Connecting agents

Your coding agents reach the app in its own terminals, with nothing installed into them. It has no model and no account of its own: the agents are the ones you already run.

## Which agents

Claude Code, Codex, Gemini CLI, GitHub Copilot, OpenCode and Hermes, started in a terminal of the window, each get [the window tool](window-tool.md):

| Agent | How it reaches the window |
| --- | --- |
| Claude Code | Its plugin, loaded from the app's copy of it (`CLAUDE_CODE_PLUGIN_DIRS`): skills, a worker subagent, and a mod that follows your comments. Claude Code 2.1.280 and later; the mod needs 2.1.287. |
| Gemini CLI, OpenCode | The window tool as an MCP server, from a settings file of the app's that their own variable names (`GEMINI_CLI_SYSTEM_DEFAULTS_PATH`, `OPENCODE_CONFIG`), merged with yours. |
| Codex, GitHub Copilot, Hermes | `llxterm` on the terminal's `PATH`, and [`llxterm guide`](window-tool.md#the-guide) for how to use it, a topic at a time. |

## Nothing is installed

The app writes nothing into an agent's settings or your home. Everything is in the environment of its terminals, so an agent started anywhere else is as you set it, and knows nothing of the window. A variable you set yourself is left as it is, and where your organization's managed settings turn Claude Code's plugin folders off, the plugin is not named.

Gemini CLI starts no MCP server in a folder you have not trusted in it: trust the project's folder there (`/permissions trust`) and the tool appears.

## What an agent tells you

An Event Center (⇧⌘E) tells you what waits for you. An agent that reports its state to its terminal (Claude Code 2.1.295 and later and Codex do, through OSC 7501, once the terminal answers that it reads it, which every terminal of the app's does) says there when it waits for you: an approval it needs ("approve Bash: npm test"), a question, a sign-in. That is one line on its terminal, named by the agent, gone as soon as it goes on; one that stopped on an error is a line among the programs that ended, and one that finished a turn is listed quietly, until you type in its terminal. While it is at work, its tab's icon carries a still ring in the side, with how far it has come when it says. With its plugin, Claude Code also says when a long turn finished, with its last line.

What waits also marks its tab with a dot and its words, counts on the Dock's icon, and, while you are in another app, comes as a silent macOS notice whose click shows the tab. A session that runs outside the window has no tab yet: its line's **Continue Here** continues it in a terminal of its project.
