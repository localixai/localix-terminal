//! Each coding agent's command line as resuming reads it: the options its own `--help` lists
//! that name a session, that are carried, and that are never carried, each list its words
//! apart by spaces. Read from Claude Code 2.1, Codex CLI 0.169, GitHub Copilot CLI, OpenCode,
//! Gemini CLI and Hermes as installed.

use super::{Check, Form, Policy};

const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const CLAUDE_MODES: &[&str] = &[
    "acceptEdits",
    "auto",
    "manual",
    "dontAsk",
    "plan",
    "bypassPermissions",
];
const SANDBOXES: &[&str] = &["read-only", "workspace-write"];
const APPROVALS: &[&str] = &["untrusted", "on-failure", "on-request", "never"];
const COPILOT_EFFORTS: &[&str] = &["none", "minimal", "low", "medium", "high", "xhigh", "max"];

pub(super) const POLICIES: [Policy; 6] = [
    Policy {
        agent: "claude",
        resume: &["--resume", "{}"],
        ids: "--resume -r --session-id",
        id_after: None,
        form: Form::Uuid,
        kept: &[
            ("--model", Check::Word),
            ("--permission-mode", Check::Choice(CLAUDE_MODES)),
            ("--add-dir", Check::Path),
            ("--agent", Check::Word),
            ("--effort", Check::Choice(EFFORTS)),
            ("--plugin-dir", Check::Path),
            ("--allow-dangerously-skip-permissions", Check::Flag),
            ("--chrome", Check::Flag),
            ("--no-chrome", Check::Flag),
            ("--ide", Check::Flag),
            ("--verbose", Check::Flag),
            ("--brief", Check::Flag),
        ],
        skips: "--dangerously-skip-permissions --permission-mode=bypassPermissions",
        bypass: "",
        once: "-p --print --no-session-persistence --bg --background --cloud --teleport \
               --desktop --environment",
        forks: "--fork-session",
        valued: "--agents --allowedTools --allowed-tools --disallowedTools --disallowed-tools \
                 --append-system-prompt --system-prompt --settings --mcp-config \
                 --setting-sources --tools --betas --fallback-model --file --debug-file \
                 --input-format --output-format --json-schema --max-budget-usd --autocompact \
                 --plugin-url --name -n --from-pr --worktree -w --debug -d --remote-control \
                 --remote-control-session-name-prefix --prompt-suggestions \
                 --system-prompt-snapshot --permission-prompts --tmux",
        commands: "agents attach auth auto-mode doctor gateway import install logs mcp plugin \
                   plugins purge respawn rm setup-token stop kill ultrareview update upgrade",
    },
    Policy {
        agent: "codex",
        resume: &["resume", "{}"],
        ids: "",
        id_after: Some("resume"),
        form: Form::Uuid,
        kept: &[
            ("-m", Check::Word),
            ("--model", Check::Word),
            ("-p", Check::Word),
            ("--profile", Check::Word),
            ("--add-dir", Check::Path),
            ("-s", Check::Choice(SANDBOXES)),
            ("--sandbox", Check::Choice(SANDBOXES)),
            ("-a", Check::Choice(APPROVALS)),
            ("--ask-for-approval", Check::Choice(APPROVALS)),
            ("--approve-for-me", Check::Flag),
            ("--search", Check::Flag),
            ("--no-alt-screen", Check::Flag),
            ("--oss", Check::Flag),
        ],
        skips: "-a=never --ask-for-approval=never",
        bypass: "--dangerously-bypass-approvals-and-sandbox -s=danger-full-access \
                 --sandbox=danger-full-access",
        once: "",
        forks: "",
        valued: "-c --config --enable --disable --remote --remote-auth-token-env -i --image \
                 --local-provider -C --cd",
        commands: "exec e review login logout mcp plugin app-server remote-control app \
                   completion update doctor sandbox debug apply a queue archive delete \
                   migrate-rollouts migrate-nfs-db unarchive fork cloud exec-server features help \
                   agents",
    },
    Policy {
        agent: "gemini",
        // Its `--resume` takes `latest` or a number, never a session's id: none is carried.
        resume: &[],
        ids: "",
        id_after: None,
        form: Form::Uuid,
        kept: &[],
        skips: "",
        bypass: "",
        once: "-p --prompt --acp --experimental-acp -l --list-extensions --list-models \
               --list-sessions --delete-session",
        forks: "",
        valued: "-r --resume -m --model -i --prompt-interactive",
        commands: "mcp extensions extension skills skill hooks hook gemma models",
    },
    Policy {
        agent: "copilot",
        resume: &["--resume={}"],
        ids: "--resume -r --session-id",
        id_after: None,
        form: Form::Uuid,
        kept: &[
            ("--model", Check::Word),
            ("--agent", Check::Word),
            ("--add-dir", Check::Path),
            ("--reasoning-effort", Check::Choice(COPILOT_EFFORTS)),
            ("--context", Check::Choice(&["default", "long_context"])),
            ("--banner", Check::Flag),
            ("--screen-reader", Check::Flag),
            ("--plain-diff", Check::Flag),
        ],
        skips: "--allow-all-tools --allow-all --yolo --allow-all-paths --allow-all-urls",
        bypass: "",
        once: "-p --prompt --connect",
        forks: "",
        valued: "-i --interactive -n --name --auto-tier --log-dir --log-level --stream \
                 --extension-sdk-path -C --attachment --disable-mcp-server \
                 --add-github-mcp-toolset --add-github-mcp-tool --additional-mcp-config \
                 --mcp-github-auth --allow-tool --deny-tool --available-tools --excluded-tools \
                 --secret-env-vars --allow-url --deny-url --max-autopilot-continues --mode \
                 --plugin-dir",
        commands: "app login help init config update version workflow sessions memories plugin \
                   mcp skill instruction lsp sandbox completion",
    },
    Policy {
        agent: "opencode",
        resume: &["--session", "{}"],
        ids: "-s --session",
        id_after: None,
        form: Form::Prefixed("ses_"),
        kept: &[
            ("-m", Check::Word),
            ("--model", Check::Word),
            ("--agent", Check::Word),
            ("--pure", Check::Flag),
            ("--mini", Check::Flag),
        ],
        skips: "--auto",
        bypass: "",
        once: "",
        forks: "--fork",
        valued: "--prompt --log-level --port --hostname --mdns-domain --cors --replay-limit",
        commands: "completion acp mcp attach run debug providers auth agent upgrade uninstall \
                   serve web models stats export import github pr session plugin plug db",
    },
    Policy {
        agent: "hermes",
        // It names no session it runs where the app can read it: none is carried.
        resume: &[],
        ids: "",
        id_after: None,
        form: Form::Uuid,
        kept: &[],
        skips: "",
        bypass: "",
        once: "-z -q",
        forks: "",
        valued: "--resume -m --provider --reasoning -t --in --skills --usage-file",
        commands: "model moa fallback worktree browser secrets egress migrate codex-runtime \
                   gateway proxy lsp setup whatsapp whatsapp-cloud slack send logout auth",
    },
];
