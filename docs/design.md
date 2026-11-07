# The app, protocol and updates (design)

What llx-desk is to an agent: one command, `llx-desk window '<json>'`, that reaches the one app
of the user, which holds the window and all its state. The desk calls no model and knows no agent:
an integration (a plugin of the agent's host) adds what that host offers on top of the same
command. Each part below is built test first; a live check uses a hidden window only.

## Layers

```text
an agent ──shell──▶ llx-desk window '<json>' ──one socket a call──▶ the app ──▶ window, tabs, PTYs
                                                                     (one per user, holds
an integration ──▶ llx-desk window --said --follow ══one socket══▶      all the state)
```

- Every agent drives the desk the same way, from its shell: a call is a process that connects,
  asks, prints the answer and exits. Nothing of an agent's host is in the crate.
- The app is the only owner of the window, and there is one per user (below).
- An integration keeps what its host needs: a follower of the person's comments
  (`--said --follow`), the board in brief at a session's start (`llx-desk window --board`), its own
  instructions (skills), its own model and settings.

## One app per user

- `serve` takes a lock at a fixed per-user path (by uid), independent of `LLX_DESK_HOME` and
  of the build, and holds it for its life. A second `serve` exits 0, quietly, when an app of
  the same build already runs; when another build runs, it starts nothing and the client reaches
  the running app (and replaces it only through the handoff below).
- Only the app creates a window and owns its life: open, show, hide, close, the tabs, the
  mini, the strip. No other process ever makes one.
- Two clients starting an app at the same moment end with one.
- `LLX_DESK_HOME` only says where files go (logs, screenshots, comments). Tests get an app with no
  window that exists in test builds alone.

## The protocol (client and app)

- JSON-RPC 2.0, one frame per line. `initialize` first and once: protocol version, build,
  capabilities, both ways. The protocol grows by optional fields and new methods, both sides
  ignore what they do not know; a breaking change is a new protocol version.
- Requests carry an `id` and may be pipelined: answers come by `id`, calls on different tabs run
  at once.
- Notifications from the server, by subscription: `comment/added`, `tab/closed`,
  `app/quitting`. No polling.
- The API describes itself (`api/describe`): every method with its params as JSON Schema, the
  events, the window tool. The adapters (the CLI, `llx-desk mcp`, a plugin) hold no list of their
  own.
- Binary payloads (screenshots) travel on the same connection, as a frame that announces its
  length, not as a temporary file.
- Each request has an idempotency key: a call retried after a broken connection is not done
  twice.
- One request's panic or failure is that request's error; the app goes on.

## What survives a break

- Comments are kept by the app until their delivery is acknowledged, so a follower that
  reconnects loses none; it subscribes again and is told them again.
- The app outlives its window: a closed window or a crashed WebView opens again on the next
  call.

## Updates

One thing moves: the app. The command, the MCP server and the Claude Code plugin are its own.

- **The app.** `install.sh` (or the release's `LLX-Desk-macos.zip`) replaces it whole. A running
  app is the old one until it quits; the next call starts the new one.
- **The command and the MCP entries** name the app's program, so they follow it; an MCP entry
  that names a program that moved is written again when the app opens (`setup::auto`).
- **The plugin** is in the app's bundle, of its version; the app opened after an update brings an
  installed plugin to that version (`claude plugin update`).
- **Client and server.** `initialize` compares protocol versions; a server of another protocol is
  named, never joined: quit it, and the next call starts this build's.

## Order

1. One server per user; a second start is quiet.
2. The protocol: `initialize`, ids and pipelining, notifications, binary screenshots, idempotent
   retries.
3. The app as the server; the API that describes itself; the adapters on it (CLI, MCP, plugin).
4. Setup from the app: the command, the plugin, the MCP hosts.
