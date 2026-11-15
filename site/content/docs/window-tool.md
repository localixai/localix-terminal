# Agents in the window

Your agents work in the window through one tool, `window`, which the app connects to each of them by itself. With it an agent opens what you would open, acts in it, and you watch.

## How an agent reaches it

Nothing to set up: the app [connects the agents you have](agents.md) when it opens, Claude Code through its plugin and the others as an MCP tool. When an agent needs the window and the app is not running, it starts in the background and never takes your keys.

## Pages

An agent opens a page beside its terminal: a dev server, a file of the project served as a site, or any address. It reads the page as an outline in which each control has a ref, and clicks, fills and picks options by those refs, never by where they are on the screen; it waits for the text that says something worked.

A script it runs in the page returns a value, cheaper and surer than a picture; a screenshot is for how it looks. It can frame the page as a phone or a tablet, as [⇧⌘M](browser.md#sizes) does for you.

What the page told besides its content comes back with it: a dialog and how it was answered (a page's `confirm` and `prompt` get Cancel unless the agent said to accept), a toast that came and went, a field that refused what was typed. A field the page refused reads `[invalid: why]`.

## Terminals

An agent opens a terminal of the project for a program it must drive: a REPL, an installer's questions, another agent. It types into it and waits for the answer. In your shell the wait ends with the command and says its exit code; lines go as one paste to a program that takes pastes. A wait on a password prompt ends at once and says so.

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
