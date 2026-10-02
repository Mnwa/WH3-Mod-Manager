//! Small modal dialogs shared by manager actions.
use super::{Manager, view::button};
use gpui_kit::{
    component::{
        WindowExt as _, button::ButtonVariants as _, input::Input, input::InputState, v_flex,
    },
    prelude::*,
    *,
};

/// Text the dialog shows; labels are already localized by the caller.
pub(super) struct TextPrompt {
    pub title: &'static str,
    pub subtitle: SharedString,
    pub placeholder: &'static str,
    pub value: String,
    pub confirm: &'static str,
}

impl Manager {
    /// One-line text dialog; `submit` runs on the manager with the entered value.
    pub(super) fn prompt_text(
        &mut self,
        prompt: TextPrompt,
        submit: impl Fn(&mut Manager, String, &mut Context<Manager>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let submit = std::rc::Rc::new(submit);
        let cancel = self.language.text("Cancel", "Отмена");
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(prompt.placeholder)
                .default_value(prompt.value)
        });
        let focus = input.clone();
        window.defer(cx, move |window, cx| {
            focus.update(cx, |input, cx| input.focus(window, cx))
        });
        let manager = cx.entity().downgrade();
        let TextPrompt {
            title,
            subtitle,
            confirm,
            ..
        } = prompt;
        window.open_dialog(cx, move |dialog, _, _| {
            let (input, manager, submit) = (input.clone(), manager.clone(), submit.clone());
            dialog
                .title(title)
                .w(px(460.))
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(crate::theme::muted())
                                .child(subtitle.clone()),
                        )
                        .child(Input::new(&input)),
                )
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            button("prompt-cancel", cancel, true)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(button("prompt-confirm", confirm, true).primary().on_click(
                            move |_, window, cx| {
                                let value = input.read(cx).value().to_string();
                                let _ = manager.update(cx, |this, cx| (*submit)(this, value, cx));
                                window.close_dialog(cx);
                            },
                        )),
                )
        });
    }

    /// Native confirmation before an action that changes Steam subscriptions.
    pub(super) fn confirm(
        &mut self,
        title: String,
        detail: &'static str,
        ok: &'static str,
        then: impl FnOnce(&mut Manager, &mut Context<Manager>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cancel = self.language.text("Cancel", "Отмена");
        let answer = window.prompt(
            PromptLevel::Warning,
            &title,
            Some(detail),
            &[ok, cancel],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                let _ = this.update(cx, |this, cx| then(this, cx));
            }
        })
        .detach();
    }
}
