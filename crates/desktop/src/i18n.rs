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

/// Supply missing Russian edit-menu translations through the kit's extension API.
pub fn install() {
    static BACKEND: std::sync::OnceLock<rust_i18n::SimpleBackend> = std::sync::OnceLock::new();
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    let backend = BACKEND.get_or_init(|| {
        let mut backend = rust_i18n::SimpleBackend::new();
        backend.add_translations(
            "ru".into(),
            [
                ("kit.Input.Cut", "Вырезать"),
                ("kit.Input.Copy", "Копировать"),
                ("kit.Input.Paste", "Вставить"),
                ("kit.Input.Select All", "Выбрать всё"),
                ("kit.Input.Go to Definition", "Перейти к определению"),
                ("kit.Input.Show Code Actions", "Показать действия"),
            ]
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect(),
        );
        backend
    });
    INSTALLED.call_once(|| gpui_kit::component::_rust_i18n_extend(backend, "kit"));
}
