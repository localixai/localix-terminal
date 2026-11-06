# desk for Claude Code

Claude opens real pages and terminals in a window you watch, and plans its work as cards on a board
there. It works the desk the way it works anything else, from its shell: `llx-desk window '<json>'`
is one call (open, interact, screenshot, close, show, list), and its answer is the tab's snapshot,
a page's outline with a ref for each control (`[e12]`) that `click`, `fill` and `select` act on.
The desk itself knows no model; this plugin is Claude Code's side of it.

## Install

1. Install **LLX Desk**, the app (https://github.com/rsaryev/llx-desk#install), and open it once.
2. In the app, **Settings → Integrations → Claude Code → Add** installs this plugin from the app
   itself, of the app's version, and keeps it in step when the app updates (`llx-desk setup
   claude-code` does the same in a terminal). The plugin's `bin/llx-desk` (on the Bash tool's
   `PATH`) runs the app's program: on the `PATH`, in `~/.local/bin`, or in the app. Nothing is
   downloaded.
3. Optional: **Hold Claude to its board card** (`finish_cards`, off by default): Claude stopping with
   its own card still in Doing is asked once to finish it first.

The skills allow `Bash(llx-desk:*)` while they are in use; allow it for the session (or in your
settings) so calls do not each ask.

## What it adds

- **Skill `window`** (the model's, hidden from the `/` menu): when to open pages and terminals and
  how, the calls and their fields.
- **Skill `board`** (the model's): when to plan work on the board, how to cut it into cards, and how
  to hand cards to subagents. It loads with the board as it is (`llx-desk window --board`).
- **Agent `llx-desk:worker`**: a subagent that takes one card, does it, and hands it in for your
  review (or blocks it with what it needs). Start several for cards that touch different files.
- **Commands**: `/tabs` (this project's tabs and your latest comments, in the chat), `/llx-desk:end`
  (close this project's tabs), `/llx-desk:stop` (quit LLX Desk).
- **A mod** (`hooks/register.tsx`):
  - follows the comments you send from the window and what you do on the board
    (`llx-desk window --said --follow --json`), and sends each to Claude as a message; a card you
    send back that a running subagent had goes to that subagent too;
  - keeps each worker's name on the board as its task gives it ("your name on the board:
    cart-button"), or gives it one from its description; a worker's call on the board without that
    name as `who` is refused with the name to pass, so it acts on its own cards and no other's;
  - a worker whose run ends with its card still in Doing is sent word once to hand it in, release
    it or block it; if it ends so again, Claude hears which card;
  - a line above the prompt: tabs, who has which card, how many wait for your review or are blocked;
  - when the session ends, closes the tabs the project opened (`llx-desk window --end`), only where
    the desk runs.

  All of it is the mod's own events (`agent.spawn`, `turn.complete`, `tool.call`, `session.end`),
  none a classic hook or the prompt's content: an organization's security default keeps those out of reach
  of the plugins a person installs.

Two sessions in one project share its tabs; use `--as NAME` to keep them apart.

## Developing

`claude --plugin-dir plugins/claude-code` runs Claude Code with this plugin;
`claude plugin validate .` checks the marketplace and `claude plugin validate plugins/claude-code` the plugin.
