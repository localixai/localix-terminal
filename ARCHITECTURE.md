# Architecture

llx-desk is one crate: a library and the `llx-desk` binary. A daemon owns the window; the CLI is a
thin client of it.

```text
llx-desk window '<json>'        the CLI (cli.rs, client.rs)
        |  JSON-RPC 2.0 lines over a unix socket (wire.rs)
        v
llx-desk serve                  the daemon (daemon.rs): lock, socket, one thread per connection
        |
        v
desk/                           the window tool's actions over tabs kept per owner
   |        |         |
   v        v         v
browser/  terminal/  goal/ + openrouter.rs
```

- **`browser/`** is the window: a `tao` event loop on the main thread, a `wry` WebView per page, a
  probe script injected in each page for observing and acting (`probe/`: the snapshot, the page's own
  requests, what the page tells, frames, the outline, refs found again, actions, typing, keys, comments,
  evaluate), the strip, the mini preview, comments, screenshots. What a page tells besides its content
  (its dialogs, answered Cancel unless a call armed OK, live-region announcements, a field that refused
  a value) comes with the probe's replies and is kept per tab across its documents (`notices.rs`).
  While the window is expanded, a click in another app collapses it to the mini (`elsewhere.rs`). `host` takes the main thread and runs the daemon on another; a `Browser` handle sends it
  commands. Panels are tabs whose page the caller paints (terminals).
- **`terminal/`** is a session: a program on a PTY, its screen kept by `vt100`, painted into a panel
  when a window shows it. Output is untrusted; only text and CSI sequences reach the emulator.
- **`desk/`** is the `window` tool. A `Call` (owner, directory, environment, key) and a JSON object in,
  text out. The registry says who owns which tab (`w1`, `t2`) and holds what the person said about it;
  nothing acts on a tab its caller does not own.
- **`goal/`, `view.rs`, `hands.rs`, `model.rs`** are the decision loop: observe a `View` (a page or a
  terminal), ask a `Decider` for one action, act, repeat, bounded and with guards against loops. It
  knows no WebView and no provider; `openrouter.rs` is the one `Decider`. `goal/commit.rs` holds back
  paying and deleting a goal does not ask for, and names the steps that may have committed something.
- **`secrets.rs`, `shapes.rs`** keep what a model and an answer must not show: saved secrets, named
  `{{secret:NAME}}` and put in by the daemon (every answer masks them), and credentials known by their
  shape. A goal sees its view through `desk/sealed.rs`, which masks both and types a secret it names.
- **`mcp/`** is `llx-desk mcp`, a client of the protocol like the CLI: `server.rs` speaks MCP on stdio (a
  tool call per thread), `catalog.rs` lists the tools, `tools.rs` turns a call into a `window` call and the
  answer into a result (a picture is image content), and `relay.rs` holds the one connection: a keeper
  thread connects, subscribes, pings and reconnects with a backoff, a call waits for it up to a limit and is
  asked again with its key.
- **`served.rs`** serves a project's files on the loopback so a path opens as a site.
- **`wire.rs`** is protocol v2: JSON-RPC 2.0, one message per line, `initialize` first. A response may
  announce bytes (`binary: {length, mime}`) that follow its line, written with it as one unit
  (`wire/pipe.rs`: a connection's one writer thread and its bounded queue). The methods
  (`daemon/methods.rs`): `window/call`, `window/end`, `window/said`, `comment/ack`, `events/subscribe`,
  `ping`, `daemon/status`, `daemon/stop`. Calls are pipelined (each `window/*` request runs on a thread of
  its own, answers come by id), a request with an `idempotencyKey` is done once (`daemon/idem.rs`, kept
  daemon-wide: the retry arrives on a new connection), a panic is that request's error. The daemon
  notifies by subscription (`daemon/hub.rs`): `comment/added` (kept by the desk until acknowledged, and
  told again on every subscribe), `tab/closed`, `window/closed`, `daemon/stopping`, and `goal/step` to the
  connection whose call is running the goal. `client.rs` and `client/link.rs` are the other end: one
  reader thread gives each answer to the call waiting for it by id and the notifications to a listener.
  The CLI, `--said --follow` and `mcp` are all clients.
- **`paths.rs`** is where the daemon lives: its lock and socket at one fixed place per user
  (`llx-desk-<uid>` in the user's temporary directory), its files in the home (`LLX_DESK_HOME`, which
  moves nothing else). A test build (feature `test-daemon`, turned on only by the crate's own dev-dependency)
  takes the place from `LLX_DESK_TEST_DIR` and never opens a window; `harness.rs` is its test support.

## Rules

- Synchronous code: threads where needed, every thread named and started with `thread::Builder`; no
  async runtime.
- A panic is a bug, and one request's panic is that request's failure, never the daemon's end
  (`catch_unwind` around each request; release builds unwind). No `unwrap`/`expect`/`panic!` in working
  code; the lints in `Cargo.toml` are hard and an exception is an `#[expect]` with its reason.
- State shared across threads is a `Mutex` or an atomic. A guard is never held across a send, a wait
  for the window, or a callback.
- The daemon starts only for a call that needs it. Ending tabs and fetching comments never start one.
- Every file opens with a `//!` that says what it is, and holds at most 300 lines of working code.
- Unit tests sit beside the code (`x.rs` ends with `#[path = "x_tests.rs"] mod tests;`). Tests that
  open a real window run only with `LLX_DESK_DISPLAY_TESTS=1`; `tests/daemon.rs` drives the binary
  against a daemon of its own with no window (a test build, in a directory of the test's own).
