# Security

The app has no approval prompt of its own: the agent's permission to use [the window](window-tool.md) is the approval. Allowing it lets an agent open pages, run scripts in them and start programs as you. Scope it as you would a shell.

## What the app listens on

Commands come only over a unix socket (0600) in a 0700 directory, and it serves only the user's own processes. Nothing listens on the network. From the first page a window opens from disk until that window closes, the app also listens on the loopback (127.0.0.1, a port the system picks) to serve that page's directory: see Pages, below.

## Pages

- Pages are untrusted. A per-run token keeps a page from answering for the probe script.
- Only `http` and `https` load, plus your project's files served on loopback under an unguessable prefix: no dot-files, nothing outside the project, and only to a request that names that address.
- A page sees anything you are signed in to in this WebView. Do not let an agent type real passwords or card numbers it can read, or buy, book or submit personal data, unless you asked for exactly that.
- Dialogs are never accepted on a page's say-so: `confirm` and `prompt` get Cancel unless the call says `dialog: "accept"`.

## Secrets

Passwords the agent should not see are [saved secrets](configuration.md#secrets): the app types the value, and answers show the placeholder. Screenshots show what the page draws.

A key an answer would carry (a GitHub, cloud, Stripe, Slack, GitLab, npm or model API key, a signed token, a private key) is shown to the agent as `[hidden: GitHub token]`, known by its shape. What you see in the window is not changed.

## Terminals and files

A terminal an agent opens runs with the agent's environment, but for the variables that only say which session called, which its integration names. Screenshots are files in the app's home; the newest 30 are kept.

## The Store

An install says what it would write or run before it does, and what an agent asks to install is shown to you the same way. See [the Store](store.md#installing).
