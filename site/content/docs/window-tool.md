# Agents in the window

Your agents work in the window through one tool, `window`, which the app hands to each of them in its own terminals. With it an agent opens what you would open, acts in it, and you watch.

## How an agent reaches it

Nothing to set up: an agent started in a terminal of the window [has it already](agents.md), Claude Code through its plugin and the others as an MCP tool or the `llxterm` command. When an agent needs the window and the app is not running, it starts in the background and never takes your keys.

## The guide

How to use the tool is one text, printed by the app's own command a topic at a time: `llxterm guide` gives the overview and the topics, and `llxterm guide commands`, `pages`, `terminals`, `panes`, `board`, `files`, `person` (your comments, its asks, secrets) and `store` each give one. An agent with no shell, only the MCP tool, reads the same topics with the tool's `guide` action. The tool's description is built from it, a short lead and a line a topic, so an agent's context holds little until it needs a topic; Claude Code's skills are short stubs of the most common calls that send it there. The text always matches the app's version.

## Short commands and steps

From a shell each action of the tool is also a short command: `llxterm open localhost:3000`, `llxterm click e12`, `llxterm fill e4 ada@example.com`, `llxterm type t1 'npm test' --enter`, `llxterm wait Saved`. A `+` between them runs several in one call, in order, each in the tab the step before used: `llxterm open localhost:3000 + click "Create account" + wait Welcome` opens a page, clicks a button by its name and checks what it shows, without reading the page first. The first step that fails or misses (a control gone, a name no control has, text that never showed) stops the rest, and the command exits 1, so `&&` stops there too. The answer is a line a step and the last step's tab once, so an agent reads less than it would in separate calls. Steps with quotes in them go one a line on stdin (`llxterm do - <<'EOF'`). Through MCP the same is the `steps` action, each step an action or a short command's string. `llxterm guide commands` lists them all.

## Pages

An agent opens a page beside its terminal: a dev server, a file of the project served as a site, or any address. It reads the page as an outline in which each control has a ref, and clicks, fills and picks options by those refs or by a control's exact name, never by where they are on the screen; it waits for the text that says something worked.

A script it runs in the page returns a value, cheaper and surer than a picture; a screenshot is for how it looks. It can frame the page as a phone or a tablet, as [⇧⌘M](browser.md#sizes) does for you.

What the page told besides its content comes back with it: a dialog and how it was answered (a page's `confirm` and `prompt` get Cancel unless the agent said to accept), a toast that came and went, a field that refused what was typed. A field the page refused reads `[invalid: why]`.

## Terminals

An agent opens a terminal of the project for a program it must drive: a REPL, an installer's questions, another agent. It types into it and waits for the answer. In your shell the wait ends with the command and says its exit code; lines go as one paste to a program that takes pastes. A wait on a password prompt ends at once and says so.

## Beside a pane

An agent can open a terminal on the right of or under a pane of the project, in that pane's tab, or a page or a file among that tab's documents: the split is drawn, and your focus and the tab you look at stay as they are. It can zoom a tab on one of its panes and back, and it is told which tab each of its panes is in and what lies beside it. Dividers and the keys between panes stay yours.

## The board

An agent plans on the project's [board](board.md): it adds cards, takes one into Doing, hands it in to Review with a note of what it did and how it checked it, or blocks it with what it needs. Subagents each take a card of their own, so two never do the same thing, and only you accept.

## The Store

An agent can search the [Store](store.md) and read a listing. What it asks to install is shown to you first, as yours is: nothing is installed on an agent's word.

## You and the agent

An agent can ask you, under the Event Center's bell, for what only you can do: sign in, a code from your phone, a decision. Your Done reaches it as your note.

What you send from the window, a comment pinned to an element or what you do on the board, reaches it with its next step; Claude Code hears it at once.

A password it should type but not see is a [saved secret](configuration.md#secrets): it asks you to save it once, the app types the value, and it only ever sees the name.

## Projects

What an agent opens belongs to the project it works in, the nearest folder with `.git` around it. It closes its tabs when it is done; Claude Code's close when its session ends.
