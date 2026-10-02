//! What the table area shows when there is no row to show: loading, first start,
//! and empty searches or filters. Each state names the next action as a button.
use super::{Filter, Manager};
use crate::theme;
use gpui_kit::{
    assets::IconName,
    component::{
        Icon, Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

const WORKSHOP: &str = "https://steamcommunity.com/app/1142710/workshop/";

/// One step of the first-start checklist; these are a real sequence, so they are numbered.
fn step(number: usize, title: &'static str, detail: &'static str) -> Div {
    div()
        .flex()
        .gap_3()
        .child(
            div()
                .size(px(24.))
                .flex_shrink_0()
                .rounded_full()
                .border_1()
                .border_color(theme::brass())
                .text_color(theme::brass())
                .text_xs()
                .flex()
                .items_center()
                .justify_center()
                .child(number.to_string()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                .child(div().text_xs().text_color(theme::muted()).child(detail)),
        )
}

fn frame() -> Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .p_6()
}

fn message(icon: IconName, title: SharedString, detail: SharedString) -> Div {
    frame()
        .child(Icon::new(icon).large().text_color(theme::muted()))
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .max_w(px(460.))
                .text_center()
                .text_color(theme::muted())
                .child(detail),
        )
}

impl Manager {
    /// First start: either the game was not found, or it has no mods yet.
    fn welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        let l = self.language;
        let game_found = self.settings.game_path.is_some();
        let (title, detail) = if game_found {
            (
                l.text("No mods installed yet", "Моды пока не установлены"),
                l.text(
                    "Subscribe to mods on the Steam Workshop. Steam downloads them, then press Rescan.",
                    "Подпишитесь на моды в Steam Workshop. Steam скачает их — затем нажмите «Пересканировать».",
                ),
            )
        } else {
            (
                l.text("Let’s find your game", "Найдём вашу игру"),
                l.text(
                    "Total War: WARHAMMER III was not found in your Steam libraries. Choose the folder that contains Warhammer3.exe.",
                    "Total War: WARHAMMER III не найдена в библиотеках Steam. Выберите папку, где лежит Warhammer3.exe.",
                ),
            )
        };
        let actions = div().flex().gap_2().child(if game_found {
            Button::new("welcome-rescan")
                .icon(IconName::RefreshCw)
                .label(l.text("Rescan", "Пересканировать"))
                .primary()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.rescan(cx)))
        } else {
            Button::new("welcome-game")
                .icon(IconName::Folder)
                .label(l.text("Choose the game folder", "Выбрать папку игры"))
                .primary()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.choose_directory(true, cx)))
        });
        let actions = if game_found {
            actions.child(
                Button::new("welcome-workshop")
                    .icon(IconName::ExternalLink)
                    .label(l.text("Open the Workshop", "Открыть Workshop"))
                    .cursor_pointer()
                    .on_click(|_, _, cx| cx.open_url(WORKSHOP)),
            )
        } else {
            actions.child(
                Button::new("welcome-folder")
                    .icon(IconName::FolderPlus)
                    .label(l.text("Add a mod folder", "Добавить папку модов"))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.choose_directory(false, cx))),
            )
        };
        frame()
            .child(img(crate::assets::LOGO).size(px(72.)))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(480.))
                    .text_center()
                    .text_color(theme::muted())
                    .child(detail),
            )
            .child(actions)
            .child(
                div()
                    .mt_6()
                    .p_4()
                    .w(px(440.))
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::panel())
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(step(
                        1,
                        l.text("Tick the mods you want", "Отметьте нужные моды"),
                        l.text(
                            "Checked mods load with the game.",
                            "Отмеченные моды загрузятся с игрой.",
                        ),
                    ))
                    .child(step(
                        2,
                        l.text("Drag to set the order", "Перетащите, чтобы задать порядок"),
                        l.text(
                            "Mods higher in the list win when they change the same thing.",
                            "Мод выше в списке побеждает, если моды меняют одно и то же.",
                        ),
                    ))
                    .child(step(
                        3,
                        l.text("Press Play", "Нажмите «Играть»"),
                        l.text(
                            "Your list is saved and the game starts with it.",
                            "Список сохранится, и игра запустится с ним.",
                        ),
                    )),
            )
            .child(
                Button::new("welcome-import")
                    .label(l.text(
                        "Used the original WH3 Mod Manager? Import its settings",
                        "Пользовались оригинальным WH3 Mod Manager? Импортируйте его настройки",
                    ))
                    .xsmall()
                    .ghost()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.import_metadata(cx))),
            )
            .into_any_element()
    }

    pub(super) fn empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let l = self.language;
        if self.busy && self.catalog.mods.is_empty() {
            return message(
                IconName::Loader,
                l.text("Reading your mods…", "Читаем ваши моды…").into(),
                l.text(
                    "Looking through the game and Workshop folders.",
                    "Просматриваем папки игры и Workshop.",
                )
                .into(),
            )
            .into_any_element();
        }
        if self.catalog.mods.is_empty() && !self.demo {
            return self.welcome(cx);
        }
        let query = self.search.read(cx).value().trim().to_owned();
        if !query.is_empty() {
            return message(
                IconName::SearchX,
                crate::ui_text!(l, "Nothing matches “{}”", "Ничего не найдено по «{}»", query).into(),
                l.text(
                    "Search looks at titles, authors, pack names and Workshop IDs. Wrap text in slashes for a /regex/.",
                    "Поиск идёт по названию, автору, имени pack и Workshop ID. Для /regex/ заключите текст в слэши.",
                )
                .into(),
            )
            .child(
                Button::new("clear-search")
                    .label(l.text("Clear search", "Очистить поиск"))
                    .small()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.search.update(cx, |input, cx| input.set_value("", window, cx));
                    })),
            )
            .into_any_element();
        }
        let (title, detail) = match self.filter {
            Filter::Enabled => (
                l.text("No mods enabled", "Нет включённых модов"),
                l.text(
                    "Tick the box next to a mod to load it with the game.",
                    "Отметьте мод галочкой, чтобы он загрузился с игрой.",
                ),
            ),
            Filter::Disabled => (
                l.text("Every mod is enabled", "Включены все моды"),
                l.text("Nothing is switched off here.", "Здесь нет отключённых модов."),
            ),
            Filter::Hidden => (
                l.text("No hidden mods", "Скрытых модов нет"),
                l.text(
                    "Right-click a mod and choose Hide from list to tidy up the library.",
                    "Чтобы убрать мод из списка, нажмите на него правой кнопкой → «Скрыть из списка».",
                ),
            ),
            Filter::All if self.category.is_some() => (
                l.text("No mods in this category", "В этой категории нет модов"),
                l.text("Pick another category on the left.", "Выберите другую категорию слева."),
            ),
            Filter::All => (
                l.text("Every mod is hidden", "Все моды скрыты"),
                l.text(
                    "Open Hidden on the left to bring them back.",
                    "Откройте «Скрытые» слева, чтобы вернуть их.",
                ),
            ),
        };
        message(IconName::Inbox, title.into(), detail.into())
            .child(
                Button::new("show-all")
                    .label(l.text("Show all mods", "Показать все моды"))
                    .small()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.filter = Filter::All;
                        this.set_category_filter(None, cx);
                    })),
            )
            .into_any_element()
    }
}
