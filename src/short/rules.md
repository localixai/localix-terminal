A page's commands also take `--dialog accept|dismiss` and `--answer TEXT`. `llxterm open` with
nothing or a folder opens the app itself, as it always did.

TAB (`w1`, `t2`) comes first, and only when what the command needs still follows it: `wait t3
ready` waits in t3 for "ready", `wait t3` waits for "t3". With none, a step works in the tab of
its kind the step before used, else the one you used last; each answer starts with its tab's id.
REF is a ref of the outline (`e12`) or a control's exact name (`click "Create account"`), so a
page you have not read is worked in one call. TEXT is one argument (several words are joined by
a space) with no escapes in it: `\n` is a backslash and an n; Enter is `--enter` or `press
Enter`. After `--` the rest of the step is values, even `+`; `-` as TEXT is read from stdin.

```sh
llxterm open localhost:3000 + click "Create account" + wait Welcome
llxterm term 'npm run dev' --wait ready + open localhost:3000
llxterm do - <<'EOF'
fill e4 "it's me" e5 '{{secret:PW}}'
click Save
wait Saved
EOF
```

`+` runs the steps in order in one call, each checked before the first runs. The first that
fails or misses (a ref gone, a name no control has, text that never showed, a script that threw)
stops the rest. The answer is a line a step and, once, the last step's tab: a page's changes
told against what you last read whole. Steps with quotes go one a line on stdin, as above. At
most 16 steps, a `shot` only last. Through MCP: `{"action": "steps", "steps": ["click e12",
"wait Saved"]}`, a step a string of these commands or an action's object.

Exit 0 done; 1 a step failed or missed (`&&` stops there); 3 no app, or a command that is not
right (nothing was done).
