use super::Manager;
use gpui_kit::*;
use wh3_core::{
    localization::Language,
    preferences::{self, Preferences},
};

impl Manager {
    pub fn language(&self) -> Language {
        self.language
    }

    fn apply_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        self.language = language;
        gpui_kit::component::set_locale(language.code());
        self.search.update(cx, |input, cx| {
            input.set_placeholder(
                language.text(
                    "Search by title, author, pack or Workshop ID (Ctrl+F)",
                    "Поиск по названию, автору, pack или Workshop ID (Ctrl+F)",
                ),
                window,
                cx,
            );
        });
        self.preset_name.update(cx, |input, cx| {
            input.set_placeholder(
                language.text("New preset name", "Название нового пресета"),
                window,
                cx,
            );
        });
        cx.notify();
    }

    pub(super) fn load_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.demo {
            self.apply_language(Language::default(), window, cx);
            return;
        }
        let handle = window.window_handle();
        self.preferences_busy = true;
        let task = cx.background_spawn(async { preferences::load(&preferences::path()?) });
        self.preferences_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = cx.update_window(handle, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.preferences_busy = false;
                    match result {
                        Ok(prefs) => {
                            this.prefs = prefs;
                            // Until the user picks a language, follow the system one.
                            let language = prefs.language.unwrap_or_else(Language::system);
                            this.apply_language(language, window, cx);
                            this.refresh_query(cx);
                        }
                        Err(error) => {
                            this.diagnostics.push(error.message());
                            this.show_report = true;
                            cx.notify();
                        }
                    }
                });
            });
        }));
    }

    pub(super) fn set_language(
        &mut self,
        language: Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preferences_busy || language == self.language {
            return;
        }
        self.apply_language(language, window, cx);
        self.update_prefs(|prefs| prefs.language = Some(language), cx);
    }

    /// Change a view preference; the list is refreshed and the file saved shortly after,
    /// so dragging a column edge writes once instead of on every frame.
    pub(super) fn update_prefs(
        &mut self,
        change: impl FnOnce(&mut Preferences),
        cx: &mut Context<Self>,
    ) {
        let before = self.prefs;
        change(&mut self.prefs);
        if before.layout != self.prefs.layout
            || before.group_by_category != self.prefs.group_by_category
        {
            self.refresh_query(cx);
        }
        cx.notify();
        if self.demo || before == self.prefs {
            return;
        }
        let executor = cx.background_executor().clone();
        self.preferences_save = Some(cx.spawn(async move |this, cx| {
            executor.timer(std::time::Duration::from_millis(400)).await;
            let Ok(prefs) = this.update(cx, |this, _| this.prefs) else {
                return;
            };
            let result = executor
                .spawn(async move { preferences::save(&preferences::path()?, &prefs) })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.status = error.message();
                    cx.notify();
                });
            }
        }));
    }
}
