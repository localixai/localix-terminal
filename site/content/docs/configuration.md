# Configuration

The app needs no configuration to run. These are the few switches it has, for secrets, for where it keeps its files, and for running with no window.

## Secrets

A password an agent should type but not see is saved once and named in a call:

```sh
llxterm secret DEMO_PW < password.txt
llxterm secret
llxterm secret --forget DEMO_PW
```

It is saved to `~/.config/llxterm/secrets/NAME` with mode 0600. Use `{{secret:NAME}}` in a page's `fill` or a terminal's `input`: the app types the value, and answers show the placeholder.

## Variables

| Variable | Meaning |
| --- | --- |
| `LLXTERM_HOME` | Where the app keeps its log, screenshots and comments. The lock and socket never move. |
| `LLXTERM_PROJECT` | The project a call belongs to, instead of the nearest directory with `.git`. |
| `LLXTERM_NO_WINDOW` | Terminals only: the app runs with no window. |
| `LLXTERM_SCRUB` | Comma-separated variables (`NAME_*` for all that start so) a terminal does not get from its caller: set by an integration for its agent's session variables. |
| `LLXTERM_VERSION` | The release the install script takes. |

## With no window

`llxterm serve --headless` keeps the window off screen until a call asks to show it; pages still render.

## Where things are kept

| Path | What |
| --- | --- |
| `~/.config/llxterm/settings.json` | Settings: the theme, the fonts, the shortcuts. |
| `~/.local/share/llxterm/llxterm.sqlite` | Projects, tabs, history, boards. |
| `~/.local/share/llxterm/catalog.sqlite` | The Store's index. |
| `.localix/board.json` in a project | The project's board, for git. |
