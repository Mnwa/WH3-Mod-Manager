//! Library search over the precomputed, lower-cased `Mod::search` text.
use super::Catalog;
use regex::{Regex, RegexBuilder};

/// Keeps an accidental pathological pattern from compiling into a huge automaton.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

enum Matcher {
    /// Every whitespace-separated term must occur somewhere in the search text.
    Terms(Vec<String>),
    Regex(Regex),
    /// Fallback for an invalid `/pattern/`, matching the original manager.
    Substring(String),
}

impl Matcher {
    fn new(query: &str) -> Self {
        let trimmed = query.trim();
        if let Some(pattern) = trimmed
            .strip_prefix('/')
            .and_then(|rest| rest.strip_suffix('/'))
        {
            // Multi-line mode lets `^`/`$` anchor to a single field (name, title, ...), because
            // the search text stores one field per line.
            return match RegexBuilder::new(pattern)
                .case_insensitive(true)
                .multi_line(true)
                .size_limit(REGEX_SIZE_LIMIT)
                .build()
            {
                Ok(regex) => Self::Regex(regex),
                Err(_) => Self::Substring(pattern.to_lowercase()),
            };
        }
        Self::Terms(
            trimmed
                .to_lowercase()
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        )
    }

    fn matches(&self, text: &str) -> bool {
        match self {
            Self::Terms(terms) => terms.iter().all(|term| text.contains(term.as_str())),
            Self::Regex(regex) => regex.is_match(text),
            Self::Substring(needle) => text.contains(needle.as_str()),
        }
    }
}

impl Catalog {
    /// Filters `order` by `query`, preserving its order.
    ///
    /// `/pattern/` is a case-insensitive regular expression, as in the original manager; an
    /// invalid pattern falls back to one plain substring: the text between the slashes. Any other
    /// query requires every whitespace-separated term to match.
    pub fn query(&self, query: &str, order: &[usize]) -> Vec<usize> {
        let matcher = Matcher::new(query);
        order
            .iter()
            .copied()
            .filter(|&index| {
                self.mods
                    .get(index)
                    .is_some_and(|item| matcher.matches(&item.search))
            })
            .collect()
    }
}
