//! "Reinstall from Workshop": unsubscribe, wait until Steam removed the files, then
//! subscribe again. It repairs downloads that "Update" cannot, as the original's
//! force-resubscribe does.
use super::Manager;
use gpui_kit::*;
use wh3_core::{message, workshop::Request};

impl Manager {
    pub(super) fn confirm_reinstall(
        &mut self,
        id: u64,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let l = self.language;
        let title = self.catalog.mods[index].title.clone();
        self.confirm(
            crate::ui_text!(
                l,
                "Reinstall “{}” from the Workshop?",
                "Переустановить «{}» из Workshop?",
                title
            ),
            l.text(
                "Steam deletes the mod's files and downloads them again. Use it when a mod is broken and Update does not help.",
                "Steam удалит файлы мода и скачает их заново. Помогает, когда мод сломан, а обновление не помогает.",
            ),
            l.text("Reinstall", "Переустановить"),
            move |this, cx| this.reinstall(id, index, cx),
            window,
            cx,
        );
    }

    fn reinstall(&mut self, id: u64, index: usize, cx: &mut Context<Self>) {
        let enabled = self.enabled.contains(&index);
        self.steam.reinstall.insert(id, enabled);
        self.steam_action(Request::Unsubscribe { ids: vec![id] }, cx);
    }

    /// After a rescan: subscribe again to reinstalled items once their files are gone.
    pub(super) fn continue_reinstall(&mut self, cx: &mut Context<Self>) {
        if self.steam.reinstall.is_empty() || !self.steam.pending.is_empty() {
            return;
        }
        let removed: Vec<(u64, bool)> = self
            .steam
            .reinstall
            .iter()
            .filter(|(id, _)| !self.steam.ids.contains_key(id))
            .map(|(&id, &enabled)| (id, enabled))
            .collect();
        if removed.is_empty() {
            // Steam kept the files (for example while the game runs); try again later.
            self.status = message!(
                "Steam has not removed the files yet; close the game and press Rescan",
                "Steam ещё не удалил файлы; закройте игру и нажмите «Пересканировать»"
            );
            return;
        }
        let ids: Vec<u64> = removed.iter().map(|(id, _)| *id).collect();
        for (id, enabled) in removed {
            self.steam.reinstall.remove(&id);
            if enabled {
                self.steam.enable_after.insert(id);
            }
        }
        self.steam_action(Request::Subscribe { ids }, cx);
    }
}
