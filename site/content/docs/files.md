# Files and the editor

The project's tree is in the side, under its tabs, live as an agent or you change it. A file picked in it opens in an editor tab.

## The tree

A folder is read when you open it and heard from the system (FSEvents, inotify) while it is in sight, so what an agent or you create, rename or delete shows as it happens, with nothing running while nothing changes. The folders you left open stay open.

A double click or Return opens a file in a tab. Its menu also offers **Open as Page** for HTML, **Open as Text** for SVG, **Open in Default App**, a terminal in a folder, and its path to copy.

The menu makes a file or a folder, names an entry anew (F2) and moves it to the Trash (⌘⌫), the name written in the row itself. Drag an entry into another folder to move it, or out over a terminal to type its path there. Nothing is ever written over, and what is refused is said under the entry.

## The editor

A file picked in the tree opens in a tab on the right of what you look at, the next one in its place. It has syntax colours for the common languages, and ⌘S saves the file as the disk wrote it, its line ends kept.

A file changed on disk is followed while you have not edited it. Where both changed, a save writes nothing and says so. What you left unsaved is kept with the file, past the tab and the app.

## What opens how

| File | Opens as |
| --- | --- |
| Text, up to 2 MB | The editor. |
| A picture (SVG too), a film, a sound, a PDF | A page that shows or plays it. |
| What no page shows (an archive, a program) | One line that says so. |
