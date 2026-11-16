The project's cards in the `b` tab: planned, taken, handed in for the person's review, shared with subagents.
The board is kept by the app and written out to `.localix/board.json`, so it outlives sessions.
Its columns:

- Backlog: ideas, not work until the person moves one to Todo; nobody takes from here.
- Todo: decided, top first; a card sent back lands at the top.
- Doing: someone works on it now; one card in Doing each, a blocked one not counted.
- Review: handed in, with a note of what was done and how it was checked.
- Done: the person accepted it. Only the person moves a card here.
- Discarded: given up. Only the person moves a card here; one of yours moved here means stop it.

`open` with `kind` board gives its tab, the same one each time; `interact` on it with an `op`
works it, and with none reads it. `llxterm window --board` prints the open cards from the file,
starting nothing.

```sh
llxterm window '{"action": "open", "kind": "board"}'
llxterm window '{"action": "interact", "tab": "b1", "op": "add", "cards": [{"title": "Cart button overlaps on mobile", "note": "Done when it does not overlap at iPhone SE."}]}'
llxterm window '{"action": "interact", "tab": "b1", "op": "take", "card": "c3"}'
llxterm window '{"action": "interact", "tab": "b1", "op": "review", "card": "c3", "note": "Moved the button to a sticky footer; checked at iPhone SE and Pixel 8."}'
llxterm window '{"action": "interact", "tab": "b1", "op": "block", "card": "c3", "reason": "Which payment provider: Stripe or Adyen?"}'
llxterm window '{"action": "interact", "tab": "b1", "op": "take", "card": "c3", "who": "cart-button"}'
```

The ops: `show` (in front of the person), `card` (one card and its history), `add` (`title` and
`note`, or `cards`; `"to": "backlog"` for an idea; `tab`: a tab it is about), `take` (`card`,
or the top of Todo: it is yours in Doing), `review` (`note`), `block` (`reason`) and `unblock`,
`release` (back to Todo, with a `note` of where it stands), `comment`, `edit`, and `remove` (a
card in Todo or Backlog).

Use it for a task of several parts the person should see and accept one by one, or for subagents
in parallel; not for a one-step task, and not as a private todo list: the person reads every
card. A card is one thing the person can check: a title of a few words, and a note with how to
tell it is done. Order Todo by what should happen first. What you find along the way goes on a
new card, not into the one you work on: in Todo when it is part of the task, in Backlog when it
is an idea the person may want later. Never work on a card in Backlog.

Subagents: cards meant to run in parallel touch different files (a shared layer first, the parts
on top of it each in its own files); cards that change the same files go to one worker, one after
the other. A subagent works as `who`, its short name, on its own cards only; the main agent on
any. Run the app's dev server once yourself and give each worker its address, so none starts its
own.

What the person does on the board (moves a card, comments, accepts, sends one back with what is
wrong) reaches you as their notes do. A card sent back waits at the top of Todo with their words
in its history: take it again and do what they said. Hand in only what you checked, and say how
in the note: the person accepts from what it says.
