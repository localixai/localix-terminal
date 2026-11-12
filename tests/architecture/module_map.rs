//! The module map as a table: what each top-level module may use of the others, written from
//! ARCHITECTURE.md, and the real dependencies it does not draw, each named with its reason.

/// What each top-level module may use of the others, from ARCHITECTURE.md's layer map: the
/// foundations and the kept owners depend on nothing above them, the window knows no domain
/// (P10), and the adapters reach the app through the client. A new module gets a row here.
pub(crate) const ALLOWED: &[(&str, &[&str])] = &[
    // Foundations: nothing of the crate.
    ("bundle", &[]),
    ("fds", &[]),
    ("out", &[]),
    ("paths", &[]),
    ("served", &[]),
    ("shots", &[]),
    ("store", &[]),
    ("view", &[]),
    ("wire", &[]),
    // What is kept, and the parts of their own: each below the desk, none knows the window.
    ("projects", &["store"]),
    ("ledger", &["projects", "store"]),
    ("layout", &["ledger", "projects", "store"]),
    ("board", &["projects", "store"]),
    ("files", &["projects", "store"]),
    ("feed", &["layout", "projects", "store"]),
    ("secrets", &["paths"]),
    ("terminal", &["fds"]),
    // An editor's page and what it is told: it knows no file but the text it is handed.
    ("editor", &[]),
    ("setup", &["bundle", "fds", "paths", "store"]),
    // The app kept up to date: it knows where the app is, and nothing of the window or the desk.
    ("update", &["bundle", "paths"]),
    // The answers are kept in the store and given to the window through `browser::Answers`.
    ("offers", &["browser", "store"]),
    // The window: Settings → Integrations reaches setup, its Updates the updater; it draws what
    // it is sent.
    (
        "browser",
        &["bundle", "paths", "setup", "shots", "update", "view"],
    ),
    // The window tool, over the window, the terminals and the kept owners.
    (
        "desk",
        &[
            "board", "browser", "editor", "feed", "files", "layout", "ledger", "offers",
            "projects", "secrets", "served", "shots", "store", "terminal", "update", "view",
        ],
    ),
    // The socket: every method onto the desk, holding the window's handle.
    (
        "server",
        &[
            "browser", "desk", "fds", "paths", "projects", "shots", "store", "wire",
        ],
    ),
    // The adapters. `cli/app.rs` is the program opened as the app: the server with its window.
    ("client", &["bundle", "paths", "projects", "wire"]),
    (
        "cli",
        &[
            "board", "browser", "bundle", "client", "follow", "mcp", "out", "paths", "secrets",
            "server", "setup", "shots",
        ],
    ),
    ("mcp", &["client", "out"]),
    ("follow", &["client"]),
    ("harness", &["client"]),
];

/// Real dependencies the module map does not draw, each with its reason. One that is gone fails
/// the test, so none outlives its reason.
pub(crate) const EXCEPTIONS: &[(&str, &str, &str)] = &[
    (
        "browser",
        "layout",
        "a split's drag stops at the layout's least share",
    ),
    (
        "client",
        "server",
        "`initialize` names this build as the server computes it",
    ),
    (
        "cli",
        "desk",
        "`window` with no JSON prints this build's tool description, starting nothing",
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
