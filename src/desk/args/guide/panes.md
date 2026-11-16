A tab split into panes: a new one opened beside another, one zoomed to fill its tab.
`open` with `beside` opens the new tab in the tab of one of the project's: a terminal on its
`side` (right when not given, or down), a page or a file among that tab's documents. The
person's focus and the tab they look at stay as they are. What you open from a terminal of the
window lands on that terminal's right without `beside`.

```sh
llxterm window '{"action": "open", "kind": "terminal", "command": "npm run dev", "beside": "t1", "side": "down"}'
llxterm window '{"action": "show", "tab": "t2", "zoom": true}'
llxterm window '{"action": "show", "tab": "t2", "zoom": false}'
```

`show` with `zoom` true lets that pane fill its tab, the tab's other panes put by and running;
false shows them again. `list` says which tab each of your panes is in and what lies beside it.
The dividers and the keys between panes are the person's.
