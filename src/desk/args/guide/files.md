A text file of the project in an editor the person sees, an `f` tab.
`open` with `path`: a UTF-8 text file of the project, up to 2 MB, relative to the caller's
directory; `line` is the line to show the person. A file has one tab, shown when opened again.
Edit the file on disk and the tab follows; `interact` reads it as the person sees it.

```sh
llxterm window '{"action": "open", "path": "src/main.rs", "line": 12}'
```
