# The browser

Pages open as tabs on the system WebView: the project's own pages, a dev server, or any site. You and an agent look at the same page.

## A page

A page has its own toolbar: back, forward, reload, the address field, its size and comments. The address field knows the project's pages and its history. ⌘L goes to it, ⌘R reloads, ⇧⌘O opens the page in your default browser.

A page that never comes, such as a dev server that has stopped, says so in one line and keeps its address, so ⌘R tries it again.

## Finding in a page

⌘F opens a find row under the page's toolbar, the same row a terminal has. What you type is searched as you type, with case ignored, in the text of the page you can see. Every match is lit, the one shown more and brought into view, and the row says which it is of how many ("3 of 12"), "No results", or "1,000+" past the most it lights.

Return or ⌘G goes to the next match, ⇧Return or ⇧⌘G to the one before. Esc closes the row and gives the keys back to the page. Each tab keeps its own row and query; a page that loads again, or another page in the tab, is searched again as it loads. The page itself is never changed: its matches are drawn over its text, so its scripts and an agent's look at it see the page as it was.

## Sizes

⇧⌘M sets the page's size: a phone, a tablet, or the tab's whole width. An agent sets it with `size` in [the window tool](window-tool.md).

## Comments

⇧⌘C pins a comment to an element of the page; ⌘↩ sends it. The agent hears it after its next call, under "What the person sent from the window", so you can point at what is wrong instead of describing it.

## The project's files

A file or a directory of the project opens as a page, served on loopback under an unguessable prefix: no dot-files, and nothing outside the project. Only `http` and `https` load besides. [Security](security.md) says what a page can and cannot do.
