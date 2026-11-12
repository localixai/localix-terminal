# The browser

Pages open as tabs on the system WebView: the project's own pages, a dev server, or any site. You and an agent look at the same page.

## A page

A page has its own toolbar: back, forward, reload, the address field, its size and comments. The address field knows the project's pages and its history. ⌘L goes to it, ⌘R reloads, ⇧⌘O opens the page in your default browser.

A page that never comes, such as a dev server that has stopped, says so in one line and keeps its address, so ⌘R tries it again.

## Sizes

⇧⌘M sets the page's size: a phone, a tablet, or the tab's whole width. An agent sets it with `size` in [the window tool](window-tool.md).

## Comments

⇧⌘C pins a comment to an element of the page; ⌘↩ sends it. The agent hears it after its next call, under "What the person sent from the window", so you can point at what is wrong instead of describing it.

## The project's files

A file or a directory of the project opens as a page, served on loopback under an unguessable prefix: no dot-files, and nothing outside the project. Only `http` and `https` load besides. [Security](security.md) says what a page can and cannot do.
