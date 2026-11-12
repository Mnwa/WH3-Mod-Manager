/// Format only the active language on UI hot paths.
#[macro_export]
macro_rules! ui_text {
    ($language:expr, $en:literal, $ru:literal, $($args:tt)+) => {
        match $language {
            wh3_core::localization::Language::English => format!($en, $($args)+),
            wh3_core::localization::Language::Russian => format!($ru, $($args)+),
        }
    };
}
