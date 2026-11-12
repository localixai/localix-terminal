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

## Keys in a terminal

| Keys | Does |
| --- | --- |
| ⌘K | Clears the screen. |
| ⌘← ⌘→ | The line's start and end. |
| ⌥← ⌥→ | A word back and forward. |
| ⌘⌫ ⌥⌫ | Deletes to the line's start, and a word. |
| ⌘↑ ⌘↓ | The top and the bottom. |
