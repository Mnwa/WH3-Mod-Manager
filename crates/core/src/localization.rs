//! Language-independent messages keep background results translatable after a locale change.
use std::{borrow::Cow, fmt};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    Russian,
}

impl Language {
    pub fn text<'a>(self, english: &'a str, russian: &'a str) -> &'a str {
        match self {
            Self::English => english,
            Self::Russian => russian,
        }
    }
    pub fn code(self) -> &'static str {
        self.text("en", "ru")
    }
    pub fn other(self) -> Self {
        match self {
            Self::English => Self::Russian,
            Self::Russian => Self::English,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Message {
    english: Cow<'static, str>,
    russian: Cow<'static, str>,
}

impl Message {
    pub fn new(
        english: impl Into<Cow<'static, str>>,
        russian: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            english: english.into(),
            russian: russian.into(),
        }
    }
    pub fn text(&self, language: Language) -> &str {
        language.text(&self.english, &self.russian)
    }
    /// Append another message per language, e.g. a localized status after a fragment.
    pub fn concat(self, other: Message) -> Self {
        Self::new(
            format!("{}{}", self.english, other.english),
            format!("{}{}", self.russian, other.russian),
        )
    }
    /// Prefix untranslated user content such as a mod name.
    pub fn prefixed(self, prefix: &str) -> Self {
        Self::new(
            format!("{prefix}{}", self.english),
            format!("{prefix}{}", self.russian),
        )
    }
}
impl From<String> for Message {
    fn from(value: String) -> Self {
        Self::new(value.clone(), value)
    }
}
impl From<&'static str> for Message {
    fn from(value: &'static str) -> Self {
        Self::new(value, value)
    }
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.english)
    }
}

/// Both format strings are checked at compile time; user content is never translated.
#[macro_export]
macro_rules! message {
    ($en:literal, $ru:literal $(,)?) => {
        $crate::localization::Message::new($en, $ru)
    };
    ($en:literal, $ru:literal, $($args:tt)+) => {
        $crate::localization::Message::new(format!($en, $($args)+), format!($ru, $($args)+))
    };
}
