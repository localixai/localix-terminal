//! The module map as a table: what each top-level module may use of the others, written from
//! ARCHITECTURE.md, and the real dependencies it does not draw, each named with its reason.

/// What each top-level module may use of the others, from ARCHITECTURE.md's layer map: the
/// foundations and the kept owners depend on nothing above them, the window knows no domain
/// (P10), and the adapters reach the app through the client. A new module gets a row here.
pub(crate) const ALLOWED: &[(&str, &[&str])] = &[
    // Foundations: nothing of the crate.
    // The person's own setup for their coding agents: their login shell's PATH and variables,
    // their programs run within a deadline, and their agents' configuration files.
    ("agents", &["fds"]),
    ("bundle", &[]),
    ("fds", &[]),
    ("fetch", &[]),
    ("kind", &[]),
    ("out", &[]),
    ("page", &[]),
    ("paths", &[]),
    ("served", &[]),
    ("shots", &[]),
    // The short commands' one table: the command line builds steps with it, the desk reads a
    // step given as a string with it.
    ("short", &[]),
    ("store", &[]),
    ("view", &[]),
    ("wire", &[]),
    // What is kept, and the parts of their own: each below the desk, none knows the window.
    ("projects", &["store"]),
    ("ledger", &["projects", "store"]),
    ("layout", &["ledger", "projects", "store"]),
    ("board", &["page", "projects", "store"]),
    ("files", &["projects", "store"]),
    ("feed", &["layout", "projects", "store"]),
    ("secrets", &["paths"]),
    // What the app's terminals are told for the agents run in them: where the program is, and
    // the app's own data folder for what it names, and a system tool run within a deadline.
    ("reach", &["bundle", "fetch", "paths"]),
    ("terminal", &["fds", "page"]),
    // An editor's page and what it is told: it knows no file but the text it is handed.
    ("editor", &["page"]),
    // The app kept up to date: it knows where the app is, and nothing of the window or the desk.
    ("update", &["bundle", "fetch", "paths"]),
    // The Store's catalogue: what is published, kept and searched, and put into the agents
    // through the places `agents` knows. It knows no window and no desk.
    (
        "catalog",
        &["agents", "fetch", "kind", "page", "paths", "store"],
    ),
    // The answers are kept in the store and given to the window through `browser::Answers`.
    ("offers", &["browser", "store"]),
    // The window: its agents are looked for on the login shell's PATH, its Updates are the
    // updater's; it draws what it is sent.
    (
        "browser",
        &[
            "agents", "bundle", "kind", "page", "paths", "shots", "update", "view",
        ],
    ),
    // The window tool, over the window, the terminals and the kept owners.
    (
        "desk",
        &[
            "board", "browser", "editor", "feed", "files", "kind", "layout", "ledger", "offers",
            "projects", "secrets", "served", "short", "shots", "store", "terminal", "update",
            "view",
        ],
    ),
    // The socket: every method onto the desk, holding the window's handle; it tells the desk
    // what the terminals are told for the agents.
    (
        "server",
        &[
            "browser", "desk", "fds", "paths", "projects", "reach", "shots", "store", "wire",
        ],
    ),
    // The adapters. `cli/app.rs` is the program opened as the app: the server with its window.
    ("client", &["bundle", "paths", "projects", "wire"]),
    (
        "cli",
        &[
            "board", "browser", "bundle", "catalog", "client", "follow", "kind", "mcp", "out",
            "paths", "secrets", "server", "short", "shots",
        ],
    ),
    ("mcp", &["client", "out"]),
    // A follower waits for the app's socket with the files' watch of its folder.
    ("follow", &["client", "files", "paths"]),
    ("harness", &["client"]),
];

/// Real dependencies the module map does not draw, each with its reason. One that is gone fails
/// the test, so none outlives its reason.
pub(crate) const EXCEPTIONS: &[(&str, &str, &str)] = &[
    (
        "browser",
        "layout",
        "the window draws a split tab by the layout's own tree and its geometry",
    ),
    (
        "client",
        "server",
        "`initialize` names this build as the server computes it",
    ),
    (
        "cli",
        "desk",
        "`window` with no JSON and `guide` print this build's tool description, starting nothing",
    ),
    (
        "mcp",
        "desk",
        "the tool's description from this build when no app answers `api/describe`",
    ),
    (
        "follow",
        "desk",
        "a note is printed as the desk renders what the person said",
    ),
];
