# Terminals and sessions

A terminal is a real shell on a PTY, and a coding agent running in one is a session. Agents stay the command-line programs their makers ship, with their own commands, sign-in and updates.

## Sessions

A terminal that runs a coding agent shows the agent's mark and, under its name, the title its session sets. The files and pages opened in a session are its own: a row of tabs on the terminal's right, with one in sight. The side lists your sessions, and each keeps its documents to itself.

A file or a page asked from no terminal joins the session you look at, else the project's last. The row scrolls sideways when its tabs do not fit, and a tab dragged along it stays where you leave it.

## Starting one

"+" in the tab row opens a terminal, or one of the agents installed on your `PATH`; ⌥⌘T opens a shell. The program is looked for where your login shell looks, not on the app's own `PATH`.

A project you come back to has a shell in each of its terminals' folders, with the last screen above.

## Colours

Terminals support ANSI colours, 256 colours and truecolor. They use their own terminal identity and colour capabilities rather than the calling agent's captured-output settings, so an inherited `NO_COLOR` or `FORCE_COLOR=0` does not make a new terminal monochrome. You can still opt out in your shell profile or for one command: `NO_COLOR=1 command`.

In the light theme, a colour a program picked for a dark ground is brought up to a contrast of 4.5:1 with the window's.

## Notifications

What a program asks its terminal to tell you (a notification: OSC 9, OSC 777) waits in the Event Center until you type in that terminal.

## Finding in the output

⌘F opens a find row under the terminal. What you type is searched as you type, with case ignored, over the scrollback and the screen; in a full-screen program (an editor, tmux) over its screen alone, which has no scrollback. Every match is lit, the one shown more, and the row says which it is of how many ("3 of 12"), or "No results".

Return or ⌘G goes to the next match, ⇧Return or ⇧⌘G to the one before. Esc closes the row and gives the keys back to the terminal; nothing typed in the row reaches the program. Find, Find Next and Find Previous are in the Edit menu, and their keys can be changed in Settings like any other.

## Keys in a terminal

| Keys | Does |
| --- | --- |
| ⌘F | Finds in the output (above). |
| ⌘K | Clears the screen at a shell. A full-screen program (an editor, tmux) keeps its own. |
| ⇧↩ | A new line in a coding agent's prompt, instead of submitting (it sends LF). At a shell, Enter. |
| ⌘← ⌘→ | The line's start and end. |
| ⌥← ⌥→ | A word back and forward. |
| ⌘⌫ ⌥⌫ | Deletes to the line's start, and a word. |
| ⌘↑ ⌘↓ | The top and the bottom. |
| ⌥ and a key, when it is Meta | Escape and the key, as shells and editors read Alt (⌥B and ⌥F move by words; ⌥↩ sends ESC CR). |
| Option-drag | Selects text, even where a program took the mouse, rather than a column. |

Option types your layout's characters (on many layouts @ [ ] { } | \ ~) until **Settings → Terminal → Option key as Meta** makes the left one, the right one or both Meta.

A program can copy to the pasteboard (OSC 52, as tmux and vim over ssh do), only from the terminal you are typing in and up to 1 MiB of text. No program can read the pasteboard.
