# Configuration

The app needs no configuration to run. These are the few things it keeps, and the switches it has.

## Secrets

A password an agent should type but not see is saved once as a secret: the agent asks you to save it, names it in what it types, and the app types the value. What the agent reads back shows only the name, never the value.

Secrets are kept in `~/.config/llxterm/secrets/`, one file each, with mode 0600.

## Variables

| Variable | Meaning |
| --- | --- |
| `LLXTERM_HOME` | Where the app keeps its log, screenshots and comments. |
| `LLXTERM_NO_WINDOW` | Terminals only: the app runs with no window. |
| `LLXTERM_VERSION` | The release the install script takes. |

## Where things are kept

| Path | What |
| --- | --- |
| `~/.config/llxterm/settings.json` | Settings: the theme, the fonts, the shortcuts. |
| `~/.local/share/llxterm/llxterm.sqlite` | Projects, tabs, history, boards. |
| `~/.local/share/llxterm/catalog.sqlite` | The Store's index. |
| `.localix/board.json` in a project | The project's board, for git. |
