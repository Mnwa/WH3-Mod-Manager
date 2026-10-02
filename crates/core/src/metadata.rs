//! Import persisted Shazbot metadata without running Electron or modifying its configuration.
use crate::{Error, Result, catalog::Catalog, error::io, preset::Preset, storage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub const FORMAT: &str = "wh3-mod-manager-metadata";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Metadata {
    pub human_name: String,
    pub author: String,
    pub workshop_id: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub req_mod_id_to_name: Vec<(String, String)>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    pub format: String,
    pub version: u32,
    pub game: String,
    pub mods: BTreeMap<String, Metadata>,
    pub presets: Vec<Preset>,
    pub current_preset: Option<Preset>,
    /// The original's `loadOrderRules`; user rules are imported by [`Bundle::user_rules`].
    #[serde(default)]
    pub load_order_rules: Value,
    /// Raw keys of switched-off pack rules (`disabledModLoadOrderRules`).
    #[serde(default)]
    pub disabled_load_order_rules: Vec<String>,
    /// Packs whose rules are switched off (`loadOrderRuleDisabledPacks`).
    #[serde(default)]
    pub load_order_rule_disabled_packs: Vec<String>,
    /// Global `alwaysEnabledModNames` and `hiddenModNames` of the original.
    #[serde(default)]
    pub always_enabled: Vec<String>,
    #[serde(default)]
    pub hidden: Vec<String>,
    /// The original's game start switches, when present in the source.
    #[serde(default)]
    pub options: Option<crate::storage::GameOptions>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Bundle {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let value: Value = serde_json::from_slice(bytes)?;
        if value.get("format").and_then(Value::as_str) == Some(FORMAT) {
            let bundle: Self = serde_json::from_value(value)?;
            if bundle.version != 1 || bundle.game != "wh3" {
                return Err(Error::Invalid(crate::message!(
                    "Unsupported metadata version or game",
                    "Неподдерживаемая версия или игра в метаданных"
                )));
            }
            return Ok(bundle);
        }
        Self::from_original(&value)
    }

    pub fn from_original(value: &Value) -> Result<Self> {
        let game = value
            .get("games")
            .and_then(|games| games.get("wh3"))
            .unwrap_or(value);
        let current = game.get("currentPreset").or_else(|| {
            value
                .get("gameToCurrentPreset")
                .and_then(|games| games.get("wh3"))
        });
        let presets = game.get("presets").or_else(|| {
            value
                .get("gameToPresets")
                .and_then(|games| games.get("wh3"))
        });
        if current.is_none() && presets.is_none() && game.get("modUserData").is_none() {
            return Err(Error::Invalid(crate::message!(
                "No WH3 metadata found in the Shazbot configuration",
                "Не найдены метаданные WH3 в конфигурации Shazbot"
            )));
        }
        let mut mods = BTreeMap::new();
        // Apply snapshots first, then the current library and canonical v3 metadata.
        if let Some(presets) = presets.and_then(Value::as_array) {
            for preset in presets {
                collect_full_mods(preset, &mut mods)?;
            }
        }
        if let Some(current) = current {
            collect_full_mods(current, &mut mods)?;
        }
        if let Some(user_data) = game.get("modUserData").and_then(Value::as_object) {
            for (name, value) in user_data {
                let data: Metadata = serde_json::from_value(value.clone())?;
                merge(mods.entry(name.clone()).or_default(), data, value);
            }
        }
        let presets: Vec<Preset> = presets
            .filter(|v| !v.is_null())
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?
            .unwrap_or_default();
        let current_preset = current
            .filter(|v| !v.is_null())
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?;
        let load_order_rules = game.get("loadOrderRules").cloned().unwrap_or(Value::Null);
        let strings = |key: &str| -> Vec<String> {
            game.get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let top = |key: &str| -> Vec<String> {
            value
                .get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let flag = |key: &str| value.get(key).and_then(Value::as_bool);
        let options =
            flag("isSkipIntroMoviesEnabled").map(|skip_intro_movies| crate::storage::GameOptions {
                skip_intro_movies,
                script_logging: flag("isScriptLoggingEnabled").unwrap_or_default(),
                auto_start_custom_battle: flag("isAutoStartCustomBattleEnabled")
                    .unwrap_or_default(),
                close_on_play: flag("isClosedOnPlay").unwrap_or_default(),
                make_units_generals: flag("isMakeUnitsGeneralsEnabled").unwrap_or_default(),
            });
        let disabled_load_order_rules = strings("disabledModLoadOrderRules");
        let load_order_rule_disabled_packs = strings("loadOrderRuleDisabledPacks");
        let warnings = vec!["Export contains only metadata saved by the original manager. Missing Workshop fields are not downloaded.".into()];
        Ok(Self {
            format: FORMAT.into(),
            version: 1,
            game: "wh3".into(),
            mods,
            presets,
            current_preset,
            load_order_rules,
            disabled_load_order_rules,
            load_order_rule_disabled_packs,
            always_enabled: top("alwaysEnabledModNames"),
            hidden: top("hiddenModNames"),
            options,
            warnings,
        })
    }
}

fn collect_full_mods(preset: &Value, mods: &mut BTreeMap<String, Metadata>) -> Result<()> {
    if let Some(entries) = preset.get("mods").and_then(Value::as_array) {
        for entry in entries {
            if let Some(name) = entry.get("name").and_then(Value::as_str) {
                let data = serde_json::from_value(entry.clone())?;
                merge(mods.entry(name.to_owned()).or_default(), data, entry);
            }
        }
    }
    Ok(())
}

fn merge(target: &mut Metadata, source: Metadata, original: &Value) {
    if !source.human_name.is_empty() {
        target.human_name = source.human_name;
    }
    if !source.author.is_empty() {
        target.author = source.author;
    }
    if !source.workshop_id.is_empty() {
        target.workshop_id = source.workshop_id;
    }
    if original.get("categories").is_some() {
        target.categories = source.categories;
    }
    if original.get("tags").is_some() {
        target.tags = source.tags;
    }
    if !source.req_mod_id_to_name.is_empty() {
        target.req_mod_id_to_name = source.req_mod_id_to_name;
    }
}

impl Bundle {
    /// The original's user rules (`{before, after, subjectPackName}` without a
    /// `sourcePackName`); pack rules are re-read from the packs themselves.
    pub fn user_rules(&self) -> Vec<crate::load_order::Rule> {
        let Some(rules) = self.load_order_rules.as_array() else {
            return vec![];
        };
        rules
            .iter()
            .filter(|rule| {
                rule.get("sourcePackName")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
            })
            .filter_map(|rule| {
                let field = |key| rule.get(key).and_then(Value::as_str);
                let (before, after) = (field("before")?, field("after")?);
                let subject = field("subjectPackName").unwrap_or(after);
                Some(crate::load_order::Rule::user(before, after, subject))
            })
            .collect()
    }
}

pub fn apply(catalog: &mut Catalog, metadata: &BTreeMap<String, Metadata>) -> usize {
    let mut matched = 0;
    for (name, data) in metadata {
        if let Some(indices) = catalog.by_name.get(&name.to_lowercase()) {
            for &index in indices {
                let item = &mut catalog.mods[index];
                if !data.human_name.is_empty() {
                    item.title = data.human_name.clone().into();
                }
                if !data.workshop_id.is_empty()
                    && data.workshop_id.bytes().all(|b| b.is_ascii_digit())
                {
                    item.workshop_id = data.workshop_id.clone().into();
                }
                item.metadata = data.clone();
                item.rebuild_search();
                matched += 1;
            }
        }
    }
    matched
}

pub fn read(path: &Path) -> Result<Bundle> {
    let size = fs::metadata(path).map_err(|e| io(path, e))?.len();
    if size > 64 * 1024 * 1024 {
        return Err(Error::Invalid(crate::message!(
            "Configuration exceeds 64 MiB",
            "Конфигурация больше 64 МБ"
        )));
    }
    Bundle::parse(&fs::read(path).map_err(|e| io(path, e))?)
}

pub fn export(source: &Path, destination: &Path) -> Result<Bundle> {
    if source == destination
        || (destination.exists()
            && fs::canonicalize(source).ok() == fs::canonicalize(destination).ok())
    {
        return Err(Error::Invalid(crate::message!(
            "Export must not overwrite the original config.json",
            "Экспорт не должен перезаписывать исходный config.json"
        )));
    }
    let bundle = read(source)?;
    storage::atomic_write(destination, &serde_json::to_vec_pretty(&bundle)?)?;
    Ok(bundle)
}

/// Localize our known export notices; preserve third-party warnings verbatim.
pub fn warning_message(warning: String) -> crate::localization::Message {
    match warning.as_str() {
        "Export contains only metadata saved by the original manager. Missing Workshop fields are not downloaded."
        | "Экспорт содержит только метаданные, сохранённые оригинальным менеджером. Отсутствующие поля Workshop не скачиваются." =>
        {
            crate::message!(
                "Export contains only metadata saved by the original manager. Missing Workshop fields are not downloaded.",
                "Экспорт содержит только метаданные, сохранённые оригинальным менеджером. Отсутствующие поля Workshop не скачиваются."
            )
        }
        "Automatic load-order rules are preserved in the file but are not yet applied by the Rust manager."
        | "Автоматические правила порядка сохранены в файле, но пока не применяются Rust-менеджером." =>
        {
            crate::message!(
                "Automatic load-order rules are preserved in the file but are not yet applied by the Rust manager.",
                "Автоматические правила порядка сохранены в файле, но пока не применяются Rust-менеджером."
            )
        }
        _ => warning.into(),
    }
}
