# Projects and windows

A project is a folder. Its sessions, tabs, history, board and events are its own, and it opens as you left it, even after a restart.

## Opening and switching

**Open Folder…** (⌘O) opens a folder as a project. The project's crumb in the bar lists every project you used, with a dot where an agent did something, and **Close Project** (⌃⌘W); ⌃⌘P opens it.

A project you come back to shows its pages, a shell in each of its terminals' folders with the last screen above, and its board. Its history (⌘Y) lists its own closed tabs.

## What an agent opens

What an agent opens belongs to the project it works in: the nearest folder with `.git` around it. [Agents in the window](window-tool.md#projects) says more.

## The side

The project's pages, terminals, board and Settings are a column on the left. Drag its edge to resize it, drag a tab to reorder, and ⌃⌘S shuts it to a rail.

## Split View

⇧⌘\, a tab's menu, or a tab dragged from the column onto the pages makes one tab of two panes: the project's two tabs side by side, listed in the column as a pair. Drag the line between them to resize, and double-click it to even them. Split proportions are kept across project switches and restarts.

## Several windows

⌘N opens another window, each with its own tabs, its own side and its own split. Drag a tab past the edge of its window and it becomes a window that follows the pointer; let go over another window's list and it goes in there. Closing a window closes its tabs.

## Putting the window away

The yellow button (or ⌘M) puts the window away: nothing of it is on screen, its pages keep drawing and your tabs stay, and the Dock icon brings it back. Nothing an agent does brings it back or puts it away. The main window's close button quits, as ⌘Q does: it asks first while terminals run a program.
