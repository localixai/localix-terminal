//! What the browser toolbar's find row asks of the page shown (`find …`, deck_find.rs), read
//! strictly: a step of its query, lit in the window's accent, or the row shut.

/// A step the toolbar's find row asks of the page shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::browser) enum Seek {
    /// The query as it is typed, the next match or the one before, lit in the window's accent
    /// (`#rrggbb`: a page cannot read the system's accent itself).
    Step {
        step: Seeking,
        accent: String,
        query: String,
    },
    /// The row shuts: the marks go, and the page has the keys again.
    Close,
}

/// Which step of the row's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::browser) enum Seeking {
    /// The query as it is typed: the match shown stays where it is while it still matches.
    Type,
    Next,
    Previous,
}

impl Seeking {
    /// The step as the page's find takes it (probe/find.js).
    pub(in crate::browser) fn word(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }
}

/// The longest query the row may send, in characters: more is no person's search.
const QUERY_MOST: usize = 1000;

impl Seek {
    /// `find close`, or `find type|next|previous #rrggbb query`, the query the rest of it
    /// (spaces and all, or none). Anything else is none.
    pub(in crate::browser) fn parse(said: &str) -> Option<Self> {
        let rest = said.strip_prefix("find ")?;
        if rest == "close" {
            return Some(Self::Close);
        }
        let (step, rest) = rest.split_once(' ')?;
        let step = match step {
            "type" => Seeking::Type,
            "next" => Seeking::Next,
            "previous" => Seeking::Previous,
            _ => return None,
        };
        let (accent, query) = rest.split_once(' ').unwrap_or((rest, ""));
        let colour = (accent.strip_prefix('#'))
            .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()));
        (colour && query.chars().count() <= QUERY_MOST).then(|| Self::Step {
            step,
            accent: accent.into(),
            query: query.into(),
        })
    }
}
