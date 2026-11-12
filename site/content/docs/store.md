# The Store

Plugins, skills and MCP servers for your coding agents in one searchable list (⇧⌘S). Nothing is installed before you are shown what it would write and run.

## What it lists

One tab in a project lists what others published for coding agents: the plugins of the Claude Code and Codex marketplaces, Agent Skills, and the servers of the MCP Registry.

The list is read from its sources when the Store is opened and at most once an hour after; the round arrow reads it now. It is kept in an index of its own (`~/.local/share/llx-desk/catalog.sqlite`) and searched there as you type, so a search never leaves your computer.

## Installing

A listing shows each agent you have and where it stands with it: Install, Update when a newer one is listed, Remove. Pick an agent: Claude Code, Codex, Gemini CLI, GitHub Copilot or OpenCode.

An install first says what it would do, the file it writes or the command it runs, and asks for what the listing needs (a key, a token), which goes only into that agent's own configuration.

| What | How it is installed |
| --- | --- |
| A skill | Copied to where the agent reads skills. |
| An MCP server | An entry in the agent's list of servers. |
| A plugin | Installed by the agent's own program. |

The Store only removes what it put there.

## From an agent

An agent can search and read the Store with [the window tool](window-tool.md). What it asks to install is shown to you the same way: nothing is installed on an agent's word.

```sh
llx-desk window '{"action": "open", "kind": "store"}'
llx-desk window '{"action": "interact", "tab": "p1", "store": {"search": "playwright"}}'
```
