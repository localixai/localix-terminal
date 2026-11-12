# The board

The project's kanban: Backlog, Todo, Doing, Review, Done and Discarded. You hand cards to agents, and only you accept what they hand in.

## A card's life

A card is taken into Doing, handed in with a note of what was done, and accepted by the person into Done. You can also drag cards, comment, edit, or send a card back to the top of Todo with what is wrong.

Each worker (the main agent, or a subagent) has at most one card in Doing, a limit you can raise, and acts only on its own cards. A card handed in goes to Review as it is.

## Running a card with an agent

A card waiting in Backlog or Todo can be run by one of your agents: ▶ on a Todo card or on the open card, or **Run With** in its menu. The card goes to Doing as the agent's, and the agent starts in a terminal of the project on a first prompt that names its card and how to hand it in.

A session in that terminal is that worker: it acts on the board under its name, and what you say of its card is told to it alone while its terminal runs.

## Where it is kept

The board is kept in the app's database (`~/.local/share/llxterm/llxterm.sqlite`), so it outlives the app and every session reads the same one. After each change it is written out to `.localix/board.json` in the project, for git and for the plugin to read.

## From an agent

```sh
llxterm window '{"action": "open", "kind": "board"}'
llxterm window '{"action": "interact", "tab": "b1", "op": "add", "cards": [{"title": "Cart button overlaps on mobile"}]}'
llxterm window '{"action": "interact", "tab": "b1", "op": "take"}'
```

The board is `b1`, the same tab each time. `op` is one of `add`, `take`, `review`, `block`, `release`, `comment`, `card`, `edit` and `remove`; with none, the call reads the board. `who` names a subagent. `llxterm window --board` prints the project's open cards from its file, starting nothing, for an agent beginning work.
