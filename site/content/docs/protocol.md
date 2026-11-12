# The app and its protocol

The app is the server: it owns the window, the terminals, the board and the database, and serves one API on a unix socket. Everything else is a thin face of that API with nothing of its own.

## One app, one API

| Face | What it is |
| --- | --- |
| `llxterm` | The command. |
| `llxterm mcp` | The window tool as an MCP server on stdio. |
| The Claude Code plugin | Runs the command. |

`api/describe` says every method, its params as JSON Schema, the events, and the window tool's description and schema, which is where `llxterm mcp` takes its tool from.

When nothing runs, the first call starts the app in the background: through LaunchServices on macOS, and elsewhere the same program as `serve`.

## The app's life

One app runs per user. Its lock and socket (`desk.lock`, `desk.sock`) live in `llxterm-<uid>` in the user's temporary directory, regardless of `LLXTERM_HOME` or `TMPDIR`, and a second start ends quietly. Its log is `app.log` in its home.

⌘Q, the main window's close button or `llxterm quit` quits it: it asks first while terminals run a program, then ends everything. The next call starts it again.

## The wire

JSON-RPC 2.0, one message per line:

- `initialize` first, then pipelined requests;
- a key per call, so a retry after a broken connection runs once;
- subscriptions: `comment/added`, `tab/closed`, `app/quitting`;
- screenshots as bytes on the same connection.

A server of another protocol is named, never joined. Only the user's own processes are served.
