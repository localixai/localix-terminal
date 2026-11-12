# Connecting agents

The app connects the coding agents you have installed by itself, and keeps them connected. It has no model and no account of its own: the agents are the ones you already run.

## Which agents

Claude Code, Codex, Gemini CLI, GitHub Copilot, OpenCode and Hermes are connected when the app opens. Each gets [the window tool](window-tool.md):

| Agent | How it is connected |
| --- | --- |
| Claude Code | Its plugin, on macOS: skills, a worker subagent, and a mod that follows your comments. |
| Codex, Gemini CLI, GitHub Copilot, OpenCode, Hermes | The window tool as an MCP server, put in with the agent's own `mcp add`. |
| Any other MCP host | The entry **Copy MCP Entry** gives, in Settings → Integrations. |

## When it happens

When the app starts, however it was started, and every hour while it runs, for whichever agent is installed. One you install later, or an entry another program rewrote, is put right without opening the app again. In a terminal, `llx-desk setup` does the same.

**Settings → Integrations** (⌘,) shows the agents. One you remove stays removed until you add it back.

## Agents with a moved configuration

An agent whose configuration you moved (`CODEX_HOME`, `GEMINI_CLI_HOME`, `COPILOT_HOME`, `XDG_CONFIG_HOME`, `HERMES_HOME`, a Hermes profile) is connected there, as your login shell sets it. An entry already in place is left as you have it.

Gemini CLI starts no MCP server in a folder you have not trusted in it: trust the project's folder there (`/permissions trust`) and the tool appears.

## What an agent tells you

An Event Center (⇧⌘E) tells you what waits for you. With its plugin, Claude Code says there when it asks you a question, stops on an error or finishes a long turn: one line on its terminal, with the agent's mark, gone as soon as it goes on.

What waits also marks its tab with a dot and its words, counts on the Dock's icon, and, while you are in another app, comes as a silent macOS notice whose click shows the tab. A session that runs outside the window has no tab yet: its line's **Continue Here** continues it in a terminal of its project.
