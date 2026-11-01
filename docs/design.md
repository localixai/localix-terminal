# The MCP connection, protocol v2 and updates (design)

What llx-desk becomes: an MCP server an agent connects to once, kept on one live socket to the
one daemon, with the CLI as the fallback. Each part below is built test first; a live check uses a
hidden window only.

## Layers

```text
Claude Code ──stdio, MCP──▶ llx-desk mcp ══one socket, for the session══▶ the daemon ──▶ window, tabs, PTYs
                            (a relay, lives as long                            (one per user, holds
                             as the session)                                    all the state)
Fallback: an agent ──shell──▶ llx-desk window '<json>' ──▶ the same daemon
```

- `llx-desk mcp` is started by the agent's host from the plugin's `.mcp.json`. On connect it
  finds the daemon, or starts it, and keeps one connection open for the whole session: no
  process per call.
- The daemon is the only owner of the window, and there is one per user (below).
- The CLI stays, for scripts and as the fallback; the skill names it only for when MCP is not
  there.

## One daemon per user

- `serve` takes a lock at a fixed per-user path (by uid), independent of `LLX_DESK_HOME` and
  of the build, and holds it for its life. A second `serve` exits 0, quietly, when a daemon of
  the same build already runs; when another build runs, it starts nothing and the client reaches
  the running daemon (and replaces it only through the handoff below).
- Only the daemon creates a window and owns its life: open, show, hide, close, the tabs, the
  mini, the strip. No other process ever makes one.
- Two clients starting a daemon at the same moment end with one.
- `LLX_DESK_HOME` only says where files go (logs, comments). Tests get a daemon with no
  window that exists in test builds alone.

## Protocol v2 (client and daemon)

- JSON-RPC 2.0, one frame per line. `initialize` first and once: protocol version, build,
  capabilities, both ways. The protocol grows by optional fields and new methods, both sides
  ignore what they do not know; a breaking change is a new protocol version.
- Requests carry an `id` and may be pipelined: answers come by `id`, calls on different tabs run
  at once.
- Notifications from the daemon, by subscription: `comment/added`, `tab/closed`, `goal/step`,
  `window/closed`, `daemon/stopping`. No polling.
- Binary payloads (screenshots) travel on the same connection, as a frame that announces its
  length, not as a temporary file.
- Each request has an idempotency key: a call retried after a broken connection is not done
  twice.
- One request's panic or failure is that request's error; the daemon goes on.

## A connection that does not drop

The stdio channel to the agent's host lives as long as the session; a break between the relay
and the daemon is never seen by the agent.

- The relay pings the daemon every 5 s. On a closed socket or no answer it reconnects with
  backoff, starting the daemon again when it is gone.
- Calls made while it reconnects wait up to a short limit, then go again; past it they fail with
  a clear error, never hang.
- Subscriptions are made again after a reconnect. Comments are kept by the daemon until their
  delivery is acknowledged, so a break loses none.
- The daemon outlives its window: a closed window or a crashed WebView opens again on the next
  call.

## MCP tools

Few, short descriptions (they ride in every session's context), annotated read-only or not.

| Tool | What it does |
|---|---|
| `open` | A page or a terminal, several at once; returns the snapshot. |
| `act` | In a tab: a `goal` with `values`, a click, typing, keys, a wait, terminal input; a goal's steps come as MCP progress. |
| `look` | The snapshot: the outline, what changed, or the whole text. |
| `screenshot` | A tab's or an element's picture, as an MCP image. |
| `tabs` | The project's tabs: list, close, show one to the person. |
| `comments` | What the person sent that was not taken yet. |

The person's comments reach the session as the server's own messages where the host takes them
(Claude Code's channels); the plugin's mod, which follows them with `window --said --follow`,
stays as the fallback while that is a preview.

## Updates

Three things move: the plugin, the binary, the running daemon.

- **Plugin and binary.** `.mcp.json` starts `${CLAUDE_PLUGIN_ROOT}/bin/launch`, not the binary.
  The launcher knows the binary version the plugin needs (`requires` in `plugin.json`); when it is
  missing it downloads that release, checks its SHA-256, and keeps it in
  `~/.llx-desk/bin/<version>/` (older ones stay, for a rollback). Offline, it runs the
  installed one when its protocol is compatible, else the MCP server says what to run.
- **The relay itself** (done). The host starts the MCP server once a session and restarts it
  neither on a plugin reload nor on a new binary. So the relay watches its own file: while no
  call runs and nothing waits on its input, a new build that has settled and answers
  `--version` is run in the relay's own process (`exec`) on the same pipes, and it tells the
  host `notifications/tools/list_changed`. A build that does not answer is refused once, said
  on stderr, and the old one goes on.
- **Relay and daemon.** `initialize` compares protocol versions; the newer side speaks the older.
- **Replacing a running daemon.** The new one asks the old for `daemon/handoff`: the old stops
  taking calls, finishes those running, writes its tabs (addresses, projects, history, comments
  not delivered) and exits; the new takes the lock and opens them again. Terminals are passed on
  where they can be (their PTYs' descriptors over the socket, `SCM_RIGHTS`), so their programs
  run on; where that fails they are closed and the handoff says so. A busy daemon (a goal
  running) is replaced once it is idle, or after a limit, never mid-step. Relays reconnect by
  themselves.
- **In Claude Code.** The marketplace updates the plugin; a new plugin version is a new
  `version`, and the launcher brings the binary with it. `/llx-desk:doctor` shows the
  versions of the plugin, the binary, the daemon and the protocol, the connection, and what is
  wrong.

## Order

1. One daemon per user; a second start is quiet.
2. Protocol v2: `initialize`, ids and pipelining, notifications, binary screenshots, idempotent
   retries.
3. `llx-desk mcp`: the relay, its reconnects, the tools.
4. The launcher and the version checks; `doctor`.
5. The handoff: tabs first, then terminals.
6. Comments as MCP messages; the skill rewritten for MCP, the CLI its fallback.
