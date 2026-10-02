use super::Manager;
use gpui_kit::*;
use wh3_core::{localization::Language, preferences};

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
                    "Search by name, pack or Workshop ID…",
                    "Поиск по названию, pack или Workshop ID…",
                ),
                window,
                cx,
            );
        });
        self.preset_name.update(cx, |input, cx| {
            input.set_placeholder(language.text("Preset name", "Название пресета"), window, cx);
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
                        Ok(language) => this.apply_language(language, window, cx),
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

    pub(super) fn change_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.preferences_busy {
            return;
        }
        let language = self.language.other();
        self.apply_language(language, window, cx);
        if self.demo {
            return;
        }
        self.preferences_busy = true;
        let task =
            cx.background_spawn(async move { preferences::save(&preferences::path()?, language) });
        self.preferences_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.preferences_busy = false;
                if let Err(error) = result {
                    this.status = error.message();
                }
                cx.notify();
            });
        }));
    }
}
