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

/// Preset version whose list order is the launch order (written by this manager).
pub const LIST_ORDER_VERSION: u32 = 3;

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
            version: Some(LIST_ORDER_VERSION),
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

    /// The launch order the entries describe, mirroring how each writer meant it.
    fn ordered_entries(&self) -> Vec<&Entry> {
        let pinned = self.mods.iter().any(|entry| entry.load_order.is_some());
        match self.version {
            // This manager writes the launch order as the list order.
            Some(version) if version >= LIST_ORDER_VERSION => self.mods.iter().collect(),
            // Earlier builds of this manager wrote version 2 without pins.
            Some(_) if !pinned => self.mods.iter().collect(),
            // The original's version 2: its launch order is `sortByNameAndLoadOrder` over the
            // enabled mods (modSortingHelpers.ts), not the list order; pins are indices among
            // enabled mods. Disabled entries follow in list order.
            Some(_) => {
                let (enabled, disabled): (Vec<_>, Vec<_>) =
                    self.mods.iter().partition(|entry| entry.is_enabled);
                let mut sorted = enabled;
                sorted.sort_by(|a, b| compare_mod_names(&a.name, &b.name));
                let mut result = splice_pins(sorted);
                result.extend(disabled);
                result
            }
            // Before version 2, the original sorted names and inserted pinned positions.
            None => {
                let mut sorted: Vec<_> = self.mods.iter().collect();
                sorted.sort_by_cached_key(|entry| entry.name.to_lowercase());
                splice_pins(sorted)
            }
        }
    }
}

/// Insert pinned entries at their `load_order` index into the name-sorted rest,
/// exactly like the original's `sortByNameAndLoadOrder`.
fn splice_pins(sorted: Vec<&Entry>) -> Vec<&Entry> {
    let total = sorted.len();
    let (mut pinned, unpinned): (Vec<_>, Vec<_>) = sorted
        .into_iter()
        .partition(|entry| entry.load_order.is_some());
    pinned.sort_by_key(|entry| entry.load_order);
    let mut pinned = pinned.into_iter().peekable();
    let mut unpinned = unpinned.into_iter();
    let mut result = Vec::with_capacity(total);
    while result.len() < total {
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

/// The original's `compareModNames`: UTF-16 code units, case-sensitive, and a
/// name that is a prefix of another sorts *after* it.
pub fn compare_mod_names(first: &str, second: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut a, mut b) = (first.encode_utf16(), second.encode_utf16());
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (Some(x), Some(y)) if x != y => return x.cmp(&y),
            _ => {}
        }
    }
}
