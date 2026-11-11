---
name: board
description: "Plan and track work on a kanban board the person watches in the llx-desk window (Backlog, Todo, Doing, Review, Done, Discarded), and hand cards to subagents: for a task of several parts, work the person wants to follow or accept piece by piece, or parallel subagents that must not do the same thing twice."
user-invocable: false
allowed-tools: Bash(llx-desk window:*)
---

# llx-desk: the board

The project's board is a tab of the llx-desk window, kept by the desk (and written out to
`.localix/board.json` in the project), so it outlives sessions. As it is now:

!`llx-desk window --board 2>&1`

(Nothing above: no open card yet.) Six columns:

| Column | What it means | Who moves cards there |
|---|---|---|
| Backlog | Ideas, not to be done yet; the person's new cards land here | anyone (`add` with `"to": "backlog"`); nobody `take`s from here |
| Todo | Decided to do, top first | anyone; **the person** moves an idea up from Backlog; a card sent back lands at the top |
| Doing | Someone works on it now | `take` |
| Review | Done, waiting for the person | `review`, with a note of what was done |
| Done | The person accepted it | **the person only** |
| Discarded | Given up, before or after it was started: not to be done | **the person only**; a card of yours moved here means stop it |

Rules the board keeps for you: each worker has at most one card in Doing (the person can raise the
limit); a card someone has is theirs alone; only the person accepts. Everything else is yours to decide.

## When to use it

- A task of several parts the person should see and accept one by one: a feature in steps, a list of bugs.
- Subagents in parallel: each takes its own card, and the board is the lock that keeps two from doing
  the same thing.
- Not for a one-step task, and not as a private todo list: the person reads every card.

## The calls

The board is a tab like a page, worked with `llx-desk window`: `open` with `kind` board gives its id
(`b1`, the same tab each time), an `interact` on it with an `op` works it, and one with no `op` reads it.

```sh
llx-desk window '{"action": "open", "kind": "board"}'
llx-desk window - <<'EOF'
{"action": "interact", "tab": "b1", "op": "add", "cards": [
  {"title": "Cart button overlaps on mobile", "note": "Below 400px. Done when it does not overlap at iPhone SE."}]}
EOF
llx-desk window '{"action": "interact", "tab": "b1", "op": "take", "card": "c3"}'
llx-desk window '{"action": "interact", "tab": "b1", "op": "review", "card": "c3", "note": "Moved the button to a sticky footer; checked at iPhone SE and Pixel 8."}'
llx-desk window '{"action": "interact", "tab": "b1", "op": "block", "card": "c3", "reason": "Which payment provider: Stripe or Adyen?"}'
llx-desk window '{"action": "interact", "tab": "b1", "op": "release", "card": "c3", "note": "Layout done, tests still fail on Safari."}'
```

Also `card` (one card and its history), `unblock`, `comment`, `edit` and `remove`; `show` brings the
board in front of the person. A note with a single quote goes on stdin (`-`), as `add` above.

## Planning

- A card is one thing the person can check: a title of a few words, and a note with how to tell it is done.
- Order Todo by what should happen first; `take` with no card takes the top.
- Put what you find along the way on a new card instead of growing the one you work on: in Todo when it
  is part of the task, in Backlog (`"to": "backlog"`) when it is an idea the person may want later.
- Never work on an idea in Backlog: it becomes work when the person moves it to Todo.

## With subagents

1. Plan the cards (`add`) so that cards meant to run in parallel touch different files: a scaffold or
   a shared layer first, then the parts on top of it, each in its own files. Then start one
   `llx-desk:worker` per card that can go in parallel; cards that change the same files go to one
   worker, one after the other.
2. Give each Agent call a short, distinct description ("cart button") and, in its prompt, the card
   and the same name as the board shows it: "take c3; your name on the board: cart-button". The
   worker passes that name as `who` in each call on the board; a call without it is refused.
3. Run the app's dev server yourself once, before the workers that need it, and give its address in
   each prompt ("the app runs at http://localhost:3000: do not start another"). Workers then neither
   start servers of their own nor leave them behind.
4. A worker that finishes with its card still in Doing is reported to you: hand it in, release it or
   block it. As the main agent you act on any card; `"who"` acts as that worker.

## What the person does

They drag cards, comment, accept a card into Done or send it back with what is wrong. That reaches you as
a message, without them typing in the chat: "The person on the board b1 returned c4 … to Todo: …". A card
sent back waits at the top of Todo with their words in its history: take it again and do what they said.
When the card was a subagent's that still runs, the note goes to that subagent too.

Hand in only what you checked, and say how in the note: the person accepts from what it says.
