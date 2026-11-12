# Getting started

Install the app, open a folder as a project, and open a terminal or start an agent in it. The app is all there is to install: it holds the window, the `llx-desk` command, the MCP server and, on macOS, the Claude Code plugin, and puts each in place for you.

> [!NOTE]
> The product is Localix Terminal. Its app and its command still carry the project's earlier name: **LLX Desk** and `llx-desk`.

## Install

macOS 12+ on Apple Silicon, or Linux with WebKitGTK.

```sh
curl -fsSL https://raw.githubusercontent.com/rsaryev/llx-desk/main/install.sh | sh
```

On macOS that is **LLX Desk.app** in `~/Applications`. Or take [`LLX-Desk-macos.dmg`](https://github.com/rsaryev/llx-desk/releases/latest/download/LLX-Desk-macos.dmg), open it and drag the app to Applications: it is signed and notarized by Apple. It is a regular Mac app, with a Dock icon, a menu bar and ⌘Q.

The script checks `SHA256SUMS` before it writes anything; `LLX_DESK_VERSION=0.1.0` picks another release.

### Linux

The program goes to `~/.local/share/llx-desk`, and `llx-desk setup command` puts the command on your `PATH` and the app among your applications. Pages need WebKitGTK 4.1 and GTK 3, glibc 2.35+ (Ubuntu 22.04+), and a display (X or Wayland). Without a display, terminals still work.

```sh
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0
```

### From source

Rust 1.89+; on Linux also `libwebkit2gtk-4.1-dev` and `libgtk-3-dev`.

```sh
cargo install --locked --git https://github.com/rsaryev/llx-desk && llx-desk setup command
```

On macOS, `cargo build --release && scripts/bundle-macos.sh` makes `target/release/bundle/macos/LLX Desk.app`, signed ad hoc for this Mac.

## Open a project

Opened, the app links `llx-desk` into `~/.local/bin` and brings the window to you.

```sh
llx-desk open .
```

That opens the current folder as a project. In the window, **Open Folder…** (⌘O) does the same. A project you come back to shows what it had open. With nothing left open, the window offers New Page, New Terminal, Board and Open Folder… with their keys, and under them the mark of each coding agent you have installed.

## Start an agent

Pick an agent's mark, or "+" in the tab row: it starts in a terminal of the project. No terminal opens unasked. The agent is already [connected to the window](agents.md), so it can open pages, drive terminals and plan on the board with [the window tool](window-tool.md).

## Updates

An app installed from a release updates itself. It downloads a newer release while you work, checks it as the script does (the checksum, and on macOS the Developer ID signature and the notarization), and installs it when you choose Restart to Update. A version that is ready is announced once, in a dialog that names both versions and lists what is new. Settings → Updates shows how far it is and can turn the looking off. A build from source is updated the way it was made.
