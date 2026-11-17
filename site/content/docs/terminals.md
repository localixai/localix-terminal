# Terminals and sessions

A terminal is a real shell on a PTY, and a coding agent running in one is a session. Agents stay the command-line programs their makers ship, with their own commands, sign-in and updates.

## How a terminal is shown

A terminal's screen is kept by a terminal core in the app, with its history, as a terminal draws it: the lines a program draws a box with, wide characters, the lines wrapped again after a resize. Agents read it and wait on it. The window draws it natively from that core, over its glass, in your theme, code font and size; a row is drawn again only when it shows something else, a terminal out of sight draws nothing, and its grid follows its pane.

Drag to select, double-click a word, triple-click a line; the wheel and the trackpad scroll the history. A right click offers Copy, Paste, Select All and Clear. A program that takes the mouse (an editor, tmux with `mouse on`) gets it. An input method (Japanese, Chinese, Korean, a dead key's accent) composes at the cursor, and the program gets only what you commit. While a program reads a password, macOS's secure input keeps other apps from reading your keys.

While ⌘ is held, an http or https address in the output, or a link a program gave (OSC 8), is underlined under the pointer; ⌘-click opens it in a tab.

A program can copy to the pasteboard (OSC 52, as tmux and vim over ssh do), only from the terminal you are typing in and up to 1 MiB of text. No program can read the pasteboard.

## Keys in a terminal

| Keys | Does |
| --- | --- |
| ⌘F | Finds in the output (below). |
| ⌘K | Clears the screen at a shell. A full-screen program (an editor, tmux) keeps its own. |
| ⇧↩ | A new line in a coding agent's prompt, instead of submitting (it sends LF). At a shell, Enter. |
| ⌘← ⌘→ | The line's start and end. |
| ⌥← ⌥→ | A word back and forward. |
| ⌘⌫ ⌥⌫ | Deletes to the line's start, and a word. |
| ⌘↑ ⌘↓ | The top and the bottom. |
| ⌥ and a key, when it is Meta | Escape and the key, as shells and editors read Alt (⌥B and ⌥F move by words; ⌥↩ sends ESC CR). |

Option types your layout's characters (on many layouts @ [ ] { } | \ ~) until **Settings → Terminal → Option key as Meta** makes the left one, the right one or both Meta.

## Sessions

A terminal that runs a coding agent shows the agent's mark and, under its name, the title its session sets. The files and pages opened in a session are its own: a row of tabs on the terminal's right, with one in sight. The side lists your sessions, and each keeps its documents to itself.

A file or a page asked from no terminal joins the session you look at, else the project's last. The row scrolls sideways when its tabs do not fit, and a tab dragged along it stays where you leave it.

## Starting one

"+" in the tab row opens a terminal, or one of the agents installed on your `PATH`; ⌥⌘T opens a shell. The program is looked for where your login shell looks, not on the app's own `PATH`.

A project you come back to has a shell in each of its terminals' folders, with the last screen above. A coding agent that was running there goes on in the same session: the app types the command that resumes it into the shell once the shell is ready, under a dimmed line that says so ("Resumed Claude Code session", and "permissions skipped" when you had skipped them). Only the agent's own options are carried: never a prompt, a one-shot run, a value that looks like a key or Codex's sandbox bypass. Claude Code, Codex, GitHub Copilot and OpenCode resume by their session's id; for Gemini CLI, Hermes, or an agent whose session is not known, the Event Center offers **Continue**, which goes on with the folder's last session. A session that does not resume is tried once more, then waits there with **Retry**. Settings → Terminal → Resume agents turns it off.

## Colours

Terminals support ANSI colours, 256 colours and truecolor. They use their own terminal identity and colour capabilities rather than the calling agent's captured-output settings, so an inherited `NO_COLOR` or `FORCE_COLOR=0` does not make a new terminal monochrome. You can still opt out in your shell profile or for one command: `NO_COLOR=1 command`.

## Notifications

What a program asks its terminal to tell you (a notification: OSC 9, OSC 777) waits in the Event Center until you type in that terminal.

A command you typed at a shell that ran 30 seconds or more, and finished while you were not looking (its terminal out of sight, its window not the one in front, or you in another app), says so there too: one line titled by its terminal, "Finished in 4m 12s" or "Failed with 2 after 45s", with the tab's dot and, while you are in another app, a silent macOS notice. A newer one takes its place, and it goes when you type in that terminal. What an agent types is not told, nor a coding agent's own run: its status says it. **Settings → Terminal → Finished commands** sets the time, from 10 seconds to 5 minutes, or turns it off.

## tmux

A terminal says it is `xterm-256color` with true colour, and `llxterm` and its version to a program that asks (XTVERSION). tmux gives a terminal it does not know the features its `TERM` names, so tell it what this one draws, in `~/.tmux.conf`:

```sh
set -g default-terminal "tmux-256color"  # where `infocmp tmux-256color` finds it; else screen-256color
set -g mouse on
set -g focus-events on                    # programs in tmux hear the window come and go
set -g set-clipboard on                   # copy mode reaches the pasteboard (OSC 52)
set -g allow-passthrough on               # a program that wraps its sequences for tmux reaches the terminal
set -as terminal-features 'xterm-256color:RGB:usstyle:overline:strikethrough:hyperlinks:sync'
```

The last line turns on curly and coloured underlines, overline, strikethrough and links (OSC 8). It is matched by `TERM`, so it applies to every terminal that says `xterm-256color`. Inside tmux the shell's command marks stay in tmux, so no finished command is told from there.

## Finding in the output

⌘F opens a find row across the foot of the terminal's pane. What you type is searched as you type, with case ignored in every script, over the scrollback and the screen; in a full-screen program (an editor, tmux) over its screen alone, which has no scrollback. Every match is lit, the one shown brighter, and the row says which it is of how many ("3 of 12"), or "No results".

Return or ⌘G goes to the next match, ⇧Return or ⇧⌘G to the one before. Esc closes the row and gives the keys back to the terminal; nothing typed in the row reaches the program. Find, Find Next and Find Previous are in the Edit menu, and their keys can be changed in Settings like any other.
