use std::fmt;

/// Who wrote a rule. User rules always win over pack rules about the same pair.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RuleSource {
    User,
    /// The pack whose `whmm\load_order.whmm` supplied the rule.
    Pack(String),
}

/// "`before` loads before `after`": `before` is placed at a lower index of the load order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    pub before: String,
    pub after: String,
    /// The pack this rule positions, and so the one `apply` prefers to move. Always `before` or
    /// `after`; anything else makes both ends movable. Distinct from `source`, which is who wrote
    /// the rule: they match for a pack rule and usually differ for a user rule.
    pub subject: String,
    pub source: RuleSource,
}

impl Rule {
    /// A rule the user made from `subject`'s side, so `subject` is the pack that gives way.
    pub fn user(before: &str, after: &str, subject: &str) -> Self {
        Self {
            before: normalize_pack_name(before),
            after: normalize_pack_name(after),
            subject: normalize_pack_name(subject),
            source: RuleSource::User,
        }
    }

    /// A rule shipped by `source`; a pack's file only ever positions that pack.
    pub fn pack(source: &str, before: &str, after: &str) -> Self {
        let source = normalize_pack_name(source);
        Self {
            before: normalize_pack_name(before),
            after: normalize_pack_name(after),
            subject: source.clone(),
            source: RuleSource::Pack(source),
        }
    }

    pub fn is_user(&self) -> bool {
        self.source == RuleSource::User
    }

    /// Identity used to switch individual pack rules off; see [`RuleKey`].
    pub fn key(&self) -> RuleKey {
        let source = match &self.source {
            RuleSource::User => String::new(),
            RuleSource::Pack(name) => pack_key(name),
        };
        RuleKey(format!(
            "{source}\t{}\t{}",
            pack_key(&self.before),
            pack_key(&self.after)
        ))
    }
}

/// Stable identity of a rule across restarts and pack updates, so "the user switched this rule
/// off" survives a re-read of someone else's pack. Same text as the original manager's
/// `loadOrderRuleKey` (`loadOrderRules.ts:97-102`): source key, before key and after key joined by
/// tabs, with an empty source for user rules. Keeping the format lets imported opt-outs match.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleKey(String);

impl RuleKey {
    /// Wraps a previously persisted key without validating it; an unknown key simply matches nothing.
    pub fn from_raw(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Accepts "foo", "foo.pack" or a whole path and returns a bare "foo.pack", keeping its case.
/// An empty result means there was no pack name at all.
pub fn normalize_pack_name(name: &str) -> String {
    let trimmed = name.trim();
    let file = trimmed
        .rfind(['/', '\\'])
        .map_or(trimmed, |separator| &trimmed[separator + 1..]);
    if file.is_empty() {
        return String::new();
    }
    if file.to_ascii_lowercase().ends_with(".pack") {
        file.to_owned()
    } else {
        format!("{file}.pack")
    }
}

/// Pack names compare case-insensitively; only the comparison is lowercased, never the stored name.
pub fn pack_key(name: &str) -> String {
    normalize_pack_name(name).to_lowercase()
}
