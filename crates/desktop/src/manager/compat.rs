//! Compatibility check of enabled mods and outdated-mod detection.
use super::{Manager, row::badge};
use crate::theme;
use gpui_kit::{assets::IconName, *};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};
use wh3_core::{
    catalog::Catalog,
    conflict::{self, Availability, ModIssues, Report},
    localization::Message,
    message, steam,
};

/// Report sections; each is rendered from lines prepared off the UI thread.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Diagnostics,
    Details,
    Files,
    Tables,
    Keys,
    Dependencies,
    Startpos,
}

#[derive(Default)]
pub(crate) struct Sections {
    pub files: Vec<Message>,
    pub tables: Vec<Message>,
    pub keys: Vec<Message>,
    pub dependencies: Vec<Message>,
    pub startpos: Vec<Message>,
    /// Workshop ids of unmet Steam requirements, for one-click subscription.
    pub required: Vec<u64>,
}

#[derive(Default)]
pub(crate) struct State {
    pub issues: HashMap<usize, ModIssues>,
    pub sections: Arc<Sections>,
    /// Outdated Workshop mods with the vanilla DB/Lua files they overwrite.
    pub outdated: HashMap<usize, Vec<String>>,
    pub checked: bool,
}

fn availability(a: Availability, catalog: &Catalog) -> Message {
    match a {
        Availability::Disabled(index) => message!(
            "installed but disabled ({})",
            "установлен, но отключён ({})",
            catalog.mods[index].name
        ),
        Availability::NotInstalled => message!("not installed", "не установлен"),
    }
}

fn sections(report: &Report, catalog: &Catalog) -> Sections {
    let name = |i: usize| catalog.mods[i].name.as_ref();
    let files = report
        .files
        .iter()
        .map(|f| {
            let others: Vec<_> = f.owners[1..].iter().map(|&i| name(i)).collect();
            message!(
                "{} · {} wins over {}",
                "{} · {} перекрывает {}",
                f.path,
                name(f.owners[0]),
                others.join(", ")
            )
        })
        .collect();
    let tables = report
        .tables
        .iter()
        .map(|t| {
            let files: Vec<_> = t
                .files
                .iter()
                .map(|f| format!("{} ({})", name(f.owner), f.path))
                .collect();
            message!(
                "db\\{} · {} colliding keys · {}",
                "db\\{} · совпадающих ключей: {} · {}",
                t.table,
                t.keys.len(),
                files.join(" > ")
            )
        })
        .collect();
    // One line per colliding row key: the game keeps only the winning file's row.
    let keys = report
        .tables
        .iter()
        .flat_map(|t| {
            t.keys.iter().map(move |key| {
                let losers: Vec<_> = key.files[1..]
                    .iter()
                    .filter_map(|&f| t.files.get(f))
                    .map(|f| name(f.owner))
                    .collect();
                message!(
                    "db\\{} [{}] · {} wins over {}",
                    "db\\{} [{}] · {} перекрывает {}",
                    t.table,
                    key.key.join(", "),
                    t.key_winner(key).map(name).unwrap_or_default(),
                    losers.join(", ")
                )
            })
        })
        .collect();
    let dependencies = report
        .missing_dependencies
        .iter()
        .map(|d| {
            let prefix = format!("{}: ", name(d.owner));
            message!("needs pack {} — ", "нужен pack {} — ", d.dependency)
                .prefixed(&prefix)
                .concat(availability(d.availability, catalog))
        })
        .chain(report.missing_required.iter().map(|r| {
            let prefix = format!("{}: ", name(r.owner));
            message!(
                "requires Workshop item {} ({}) — ",
                "требует мод Workshop {} ({}) — ",
                r.name,
                r.workshop_id
            )
            .prefixed(&prefix)
            .concat(availability(r.availability, catalog))
        }))
        .collect();
    let startpos = report
        .startpos
        .iter()
        .map(|&i| {
            message!(
                "{}: changes the campaign start position together with other enabled mods",
                "{}: меняет стартовую позицию кампании вместе с другими включёнными модами",
                name(i)
            )
        })
        .collect();
    let mut required: Vec<u64> = report
        .missing_required
        .iter()
        .filter_map(|r| r.workshop_id.parse().ok())
        .collect();
    required.sort_unstable();
    required.dedup();
    Sections {
        files,
        tables,
        keys,
        dependencies,
        startpos,
        required,
    }
}

impl Manager {
    pub(super) fn check(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.demo {
            return;
        }
        self.busy = true;
        self.cancellable = true;
        self.status = message!(
            "Checking enabled mods for compatibility…",
            "Проверка совместимости включённых модов…"
        );
        self.cancel.store(false, Ordering::Relaxed);
        let (catalog, order, enabled, cancel) = (
            self.catalog.clone(),
            self.order.clone(),
            self.enabled.clone(),
            self.cancel.clone(),
        );
        let task = cx.background_spawn(async move {
            let report = conflict::check(&catalog, &order, &enabled, &cancel)?;
            let issues = report.issues_by_mod();
            Ok::<_, wh3_core::Error>((sections(&report, &catalog), issues, report.warnings))
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.cancellable = false;
                match result {
                    Ok((sections, issues, warnings)) => {
                        this.status = message!(
                            "Overwritten files: {} · colliding DB keys: {} · dependency problems: {} · startpos: {}. An overlap does not always mean incompatibility.",
                            "Перезаписанных файлов: {} · совпадающих DB-ключей: {} · проблем зависимостей: {} · startpos: {}. Совпадение не всегда означает несовместимость.",
                            sections.files.len(),
                            sections.keys.len(),
                            sections.dependencies.len(),
                            sections.startpos.len()
                        );
                        this.report_tab = if !sections.dependencies.is_empty() { Tab::Dependencies } else { Tab::Files };
                        this.compat = State {
                            issues,
                            sections: Arc::new(sections),
                            outdated: std::mem::take(&mut this.compat.outdated),
                            checked: true,
                        };
                        this.diagnostics.extend(warnings);
                        this.show_report = true;
                    }
                    Err(error) => this.status = error.message(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Flag Workshop mods that are older than the game's last Steam update and
    /// overwrite vanilla DB or Lua files, as the original's "outdated overwriting packs".
    pub(super) fn refresh_outdated(&mut self, cx: &mut Context<Self>) {
        let (Some(game), false) = (self.settings.game_path.clone(), self.demo) else {
            return;
        };
        let (catalog, cancel) = (self.catalog.clone(), self.cancel.clone());
        // Vanilla packs only change with the game, so their paths are read once per folder.
        let cached = self
            .launch
            .vanilla
            .clone()
            .filter(|(path, _)| *path == game);
        let task = cx.background_spawn(async move {
            let Some(updated) = steam::game_updated(&game) else {
                return Ok((HashMap::new(), None));
            };
            let vanilla = match cached {
                Some((_, vanilla)) => vanilla,
                None => Arc::new(wh3_core::outdated::vanilla_files(
                    &game.join("data"),
                    &cancel,
                )?),
            };
            let outdated = wh3_core::outdated::overwriting(&catalog, updated, &vanilla, &cancel)?;
            Ok::<_, wh3_core::Error>((outdated, Some((game, vanilla))))
        });
        self.launch.outdated_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((outdated, vanilla)) => {
                        this.compat.outdated = outdated;
                        if vanilla.is_some() {
                            this.launch.vanilla = vanilla;
                        }
                    }
                    Err(error) => this.diagnostics.push(error.message()),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn compat_badges(&self, index: usize) -> Vec<AnyElement> {
        let Some(issues) = self.compat.issues.get(&index) else {
            return vec![];
        };
        let l = self.language;
        let mut badges = Vec::new();
        if issues.missing_dependencies + issues.missing_required > 0 {
            badges.push(
                badge(
                    ("missing", index),
                    IconName::CircleAlert,
                    theme::danger(),
                    crate::ui_text!(
                        l,
                        "Missing dependencies: {}",
                        "Не хватает зависимостей: {}",
                        issues.missing_dependencies + issues.missing_required
                    ),
                )
                .into_any_element(),
            );
        }
        if issues.overwritten + issues.keys_overwritten > 0 || issues.startpos {
            badges.push(
                badge(("conflict", index), IconName::TriangleAlert, theme::warning(), crate::ui_text!(
                    l,
                    "Files overwritten by higher-priority mods: {} · DB rows overridden: {} · this mod overrides {} files and {} rows{}",
                    "Перекрыто модами с большим приоритетом: файлов {}, DB-строк {} · этот мод перекрывает файлов {} и строк {}{}",
                    issues.overwritten,
                    issues.keys_overwritten,
                    issues.overwrites,
                    issues.keys_overwrites,
                    if issues.startpos { " · startpos" } else { "" }
                ))
                .into_any_element(),
            );
        }
        badges
    }
}
