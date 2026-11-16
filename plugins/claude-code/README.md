# Localix Terminal for Claude Code

Claude opens real pages and terminals in the Localix Terminal window you watch, beside the
terminal it runs in, and plans its work as cards on the project's board there. It reads a page
as an outline with a ref for each control, and clicks and fills by those refs. The app knows no
model; this plugin is Claude Code's side of it.

## Where it runs

The plugin is part of **Localix Terminal** (https://github.com/localixai/localix-terminal#install)
and is never installed: Claude Code 2.1.280 and later, started in a terminal of the app, loads it
from the app's copy of it (`CLAUDE_CODE_PLUGIN_DIRS`), of the app's version; its mod needs
2.1.287. Elsewhere Claude Code knows nothing of it. The plugin reaches the app through the
app's own program, which those terminals put first on the `PATH`.

**Hold Claude to its board card** (`finish_cards`, off by default): Claude stopping with its own
card still in Doing is asked once to finish it first.

The skills allow `Bash(llxterm:*)` while they are in use; allow it for the session (or in your
settings) so calls do not each ask.

## What it adds

- **Skill `window`** (the model's, hidden from the `/` menu): when to open pages and terminals and
  how, the calls and their fields.
- **Skill `board`** (the model's): when to plan work on the board, how to cut it into cards, and how
  to hand cards to subagents. It loads with the board as it is.
- **Agent `llxterm:worker`**: a subagent that takes one card, does it, and hands it in for your
  review (or blocks it with what it needs). Start several for cards that touch different files.
- **Commands**: `/tabs` (this project's tabs and your latest comments, in the chat), `/llxterm:end`
  (close this project's tabs), `/llxterm:stop` (quit Localix Terminal).
- **A mod** (`hooks/register.tsx`):
  - follows the app through one `llxterm window --watch`, which prints a line as the app comes
    or goes, a tab opens, changes or closes, the board changes or you comment: nothing is asked
    again and again;
  - sends the comments you send from the window and what you do on the board to Claude as
    messages, as they come; a card you send back that a running subagent had goes to that
    subagent too;
  - keeps each worker's name on the board as its task gives it ("your name on the board:
    cart-button"), or gives it one from its description; a worker's call on the board without that
    name as `who` is refused with the name to pass, so it acts on its own cards and no other's;
  - a worker whose run ends with its card still in Doing is sent word once to hand it in, release
    it or block it; if it ends so again, Claude hears which card;
  - a line above the prompt: tabs, who has which card, how many wait for your review or are blocked;
  - when the session ends, closes the tabs the project opened, only while
    the app runs;
  - in a terminal of Localix Terminal (`TERM_PROGRAM=llxterm`), tells Claude that a page you ask for
    ("open google") opens as a tab of the window, not in another browser it has the tools of (Claude
    in Chrome, a built-in browser pane), unless you name that browser.

  All of it is the mod's own events (`agent.spawn`, `turn.complete`, `tool.call`, `session.end`),
  none a classic hook or the prompt's content: an organization's security default keeps those out
  of reach of the plugins a person installs. The window as the browser is one row the mod adds
  to the conversation when the session starts, which Claude reads and you do not see.

Two sessions in one project share its tabs; use `--as NAME` to keep them apart.

## Developing

`claude --plugin-dir plugins/claude-code` runs Claude Code with this plugin;
`claude plugin validate plugins/claude-code` checks it.
`bun test plugins/claude-code/hooks` runs the hooks' tests (`register.spec.ts`, against a mocked
host).
