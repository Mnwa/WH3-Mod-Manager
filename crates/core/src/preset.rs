use crate::{Error, Result, catalog::Catalog};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub mods: Vec<Entry>,
    #[serde(default)]
    pub version: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    #[serde(default = "enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub load_order: Option<usize>,
}
fn enabled() -> bool {
    true
}

pub struct Applied {
    pub order: Vec<usize>,
    pub enabled: HashSet<usize>,
    pub missing: Vec<String>,
}

impl Preset {
    pub fn parse(bytes: &[u8]) -> Result<Vec<Self>> {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        let presets: Vec<Self> = if value.is_array() {
            serde_json::from_value(value)?
        } else if let Some(presets) = value
            .get("presets")
            .or_else(|| value.pointer("/games/wh3/presets"))
            .or_else(|| value.pointer("/gameToPresets/wh3"))
        {
            serde_json::from_value(presets.clone())?
        } else {
            vec![serde_json::from_value(value)?]
        };
        if presets.iter().any(|preset| preset.name.trim().is_empty()) {
            return Err(Error::Invalid(crate::message!(
                "Preset name is missing",
                "У пресета отсутствует имя"
            )));
        }
        Ok(presets)
    }

    pub fn capture(
        name: String,
        catalog: &Catalog,
        order: &[usize],
        enabled: &HashSet<usize>,
    ) -> Self {
        Self {
            name,
            version: Some(2),
            mods: order
                .iter()
                .filter_map(|&index| {
                    catalog.mods.get(index).map(|item| Entry {
                        name: item.name.to_string(),
                        is_enabled: enabled.contains(&index),
                        load_order: None,
                    })
                })
                .collect(),
        }
    }

    pub fn apply(&self, catalog: &Catalog) -> Applied {
        let mut result = Applied {
            order: Vec::with_capacity(catalog.mods.len()),
            enabled: HashSet::new(),
            missing: vec![],
        };
        let mut seen = HashSet::new();
        let entries = self.ordered_entries();
        for entry in entries {
            match catalog
                .by_name
                .get(&entry.name.to_lowercase())
                .and_then(|indices| indices.first())
                .copied()
            {
                Some(index) if seen.insert(index) => {
                    result.order.push(index);
                    if entry.is_enabled {
                        result.enabled.insert(index);
                    }
                }
                Some(_) => {}
                None => result.missing.push(entry.name.clone()),
            }
        }
        result
            .order
            .extend((0..catalog.mods.len()).filter(|index| !seen.contains(index)));
        result
    }

    fn ordered_entries(&self) -> Vec<&Entry> {
        if self.version.is_some() {
            return self.mods.iter().collect();
        }
        // Before version 2, the original sorted names and inserted pinned positions.
        let mut sorted: Vec<_> = self.mods.iter().collect();
        sorted.sort_by_cached_key(|entry| entry.name.to_lowercase());
        let (mut pinned, unpinned): (Vec<_>, Vec<_>) = sorted
            .into_iter()
            .partition(|entry| entry.load_order.is_some());
        pinned.sort_by_key(|entry| entry.load_order);
        let mut pinned = pinned.into_iter().peekable();
        let mut unpinned = unpinned.into_iter();
        let mut result = Vec::with_capacity(self.mods.len());
        while result.len() < self.mods.len() {
            while pinned
                .peek()
                .is_some_and(|entry| entry.load_order.unwrap_or(0) <= result.len())
            {
                if let Some(entry) = pinned.next() {
                    result.push(entry);
                }
            }
            if let Some(entry) = unpinned.next().or_else(|| pinned.next()) {
                result.push(entry);
            }
        }
        result
    }
}
