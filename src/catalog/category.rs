//! The categories the Store shows, one fixed list whatever the sources call theirs. A listing's
//! category is its source's own, mapped through one table; one the table does not know, or a
//! listing that names none, is placed by the whole words of its name and summary against a
//! short list of words for each category; what matches none is Other. It runs for every
//! listing the index writes, so it allocates nothing per word: each word is lowered into a
//! buffer on the stack and looked up in one sorted table.

use std::sync::OnceLock;

/// Every category, in the order the Store offers them, Other last.
pub const ALL: [&str; 11] = [
    "Development",
    "Data",
    "Content",
    "Productivity",
    "Design",
    "Security",
    "Operations",
    "Learning",
    "Games",
    "Utilities",
    "Other",
];

/// What a listing that fits nothing is.
const OTHER: &str = "Other";

/// The categories the sources give, lowercase, and the one each is shown as.
pub(crate) const GIVEN: [(&str, &str); 42] = [
    ("development", "Development"),
    ("developer tools", "Development"),
    ("developer-tools", "Development"),
    ("coding", "Development"),
    ("testing", "Development"),
    ("migration", "Development"),
    ("database", "Data"),
    ("data", "Data"),
    ("analytics", "Data"),
    ("data & analytics", "Data"),
    ("finance", "Data"),
    ("math", "Data"),
    ("content", "Content"),
    ("writing", "Content"),
    ("media", "Content"),
    ("documentation", "Content"),
    ("productivity", "Productivity"),
    ("automation", "Productivity"),
    ("workflow", "Productivity"),
    ("communication", "Productivity"),
    ("business & operations", "Productivity"),
    ("business", "Productivity"),
    ("design", "Design"),
    ("creativity", "Design"),
    ("security", "Security"),
    ("deployment", "Operations"),
    ("monitoring", "Operations"),
    ("infrastructure", "Operations"),
    ("devops", "Operations"),
    ("operations", "Operations"),
    ("learning", "Learning"),
    ("education", "Learning"),
    ("education & research", "Learning"),
    ("research", "Learning"),
    ("scientific research", "Learning"),
    ("games", "Games"),
    ("game", "Games"),
    ("entertainment", "Games"),
    ("utilities", "Utilities"),
    ("utility", "Utilities"),
    ("tools", "Utilities"),
    ("location", "Utilities"),
];

/// The words that place a listing in a category, the most specific category first: on a tie
/// the first wins.
pub(crate) const KEYWORDS: [(&str, &[&str]); 9] = [
    (
        "Security",
        &[
            "security",
            "secure",
            "vulnerability",
            "vulnerabilities",
            "secret",
            "secrets",
            "auth",
            "oauth",
            "pentest",
            "audit",
            "cve",
            "owasp",
            "malware",
            "threat",
            "encryption",
        ],
    ),
    (
        "Operations",
        &[
            "kubernetes",
            "k8s",
            "docker",
            "deploy",
            "deployment",
            "terraform",
            "aws",
            "gcp",
            "azure",
            "ci",
            "monitoring",
            "logs",
            "observability",
            "devops",
            "infrastructure",
        ],
    ),
    (
        "Data",
        &[
            "database",
            "sql",
            "postgres",
            "postgresql",
            "mysql",
            "sqlite",
            "mongodb",
            "analytics",
            "bigquery",
            "snowflake",
            "csv",
            "spreadsheet",
            "excel",
            "etl",
            "dataset",
        ],
    ),
    (
        "Design",
        &[
            "design",
            "designs",
            "figma",
            "ui",
            "ux",
            "css",
            "tailwind",
            "sketch",
            "icons",
            "typography",
            "wireframe",
            "canva",
        ],
    ),
    (
        "Content",
        &[
            "writing",
            "blog",
            "content",
            "copywriting",
            "seo",
            "documentation",
            "markdown",
            "pdf",
            "video",
            "image",
            "images",
            "podcast",
            "social",
        ],
    ),
    (
        "Learning",
        &[
            "learn",
            "learning",
            "tutorial",
            "course",
            "teach",
            "teaching",
            "education",
            "quiz",
            "study",
            "students",
            "research",
        ],
    ),
    (
        "Games",
        &[
            "game",
            "games",
            "gaming",
            "unity",
            "godot",
            "chess",
            "minecraft",
            "steam",
            "puzzle",
        ],
    ),
    (
        "Productivity",
        &[
            "productivity",
            "calendar",
            "email",
            "gmail",
            "slack",
            "notion",
            "tasks",
            "todo",
            "meeting",
            "meetings",
            "notes",
            "jira",
            "linear",
            "workflow",
            "automation",
        ],
    ),
    (
        "Development",
        &[
            "code",
            "coding",
            "developer",
            "developers",
            "git",
            "github",
            "api",
            "sdk",
            "testing",
            "debug",
            "refactor",
            "typescript",
            "python",
            "rust",
            "review",
        ],
    ),
];

/// The longest keyword, in bytes: a longer word is none.
pub(crate) const LONGEST: usize = 16;

/// The category `given` by a source, of a listing called `name` that does what `summary` says.
pub fn of(given: &str, name: &str, summary: &str) -> &'static str {
    let given = given.trim();
    let mapped = GIVEN
        .iter()
        .find(|(said, _)| said.eq_ignore_ascii_case(given));
    if let Some((_, mapped)) = mapped {
        return mapped;
    }
    placed(name, summary)
}

/// Every keyword with the place of its category in [`KEYWORDS`], sorted for a binary search.
fn table() -> &'static [(&'static str, usize)] {
    static TABLE: OnceLock<Vec<(&'static str, usize)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: Vec<(&'static str, usize)> = (KEYWORDS.iter().enumerate())
            .flat_map(|(at, (_, words))| words.iter().map(move |word| (*word, at)))
            .collect();
        table.sort_unstable();
        table
    })
}

/// The category the words of `name` and `summary` say most of, the first on a tie; Other when
/// none of them is a keyword.
fn placed(name: &str, summary: &str) -> &'static str {
    let table = table();
    let mut counts = [0_u32; KEYWORDS.len()];
    let mut buffer = [0_u8; LONGEST];
    let words = (name.split(|c: char| !c.is_alphanumeric()))
        .chain(summary.split(|c: char| !c.is_alphanumeric()));
    for word in words {
        if word.is_empty() || word.len() > LONGEST || !word.is_ascii() {
            continue;
        }
        let Some(lowered) = buffer.get_mut(..word.len()) else {
            continue;
        };
        lowered.copy_from_slice(word.as_bytes());
        lowered.make_ascii_lowercase();
        let lowered = &*lowered;
        if let Ok(found) = table.binary_search_by(|(keyword, _)| keyword.as_bytes().cmp(lowered))
            && let Some((_, at)) = table.get(found)
            && let Some(count) = counts.get_mut(*at)
        {
            *count += 1;
        }
    }
    let mut best: Option<(usize, u32)> = None;
    for (at, count) in counts.into_iter().enumerate() {
        if count > 0 && best.is_none_or(|(_, most)| count > most) {
            best = Some((at, count));
        }
    }
    best.and_then(|(at, _)| KEYWORDS.get(at))
        .map_or(OTHER, |(category, _)| category)
}

#[cfg(test)]
#[path = "category_tests.rs"]
mod tests;
