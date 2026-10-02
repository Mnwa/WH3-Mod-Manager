//! The original manager's "Share Mod List" text, so lists can be swapped with its users.
//!
//! Entries are separated by `|`. A Workshop mod is its id; any other mod is
//! `local:<URI-encoded pack name>[:<Workshop id>]`. An optional `;<load order>` pins it.
use crate::{
    catalog::{Catalog, Source},
    preset::{Entry, Preset},
};
use std::collections::{HashMap, HashSet};

const LOCAL_PREFIX: &str = "local:";

/// One mod from a shared list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shared {
    /// Empty for a local mod without a known Workshop copy.
    pub workshop_id: String,
    pub name: Option<String>,
    pub load_order: Option<usize>,
}

fn is_workshop_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

/// `encodeURIComponent`: everything but `A-Z a-z 0-9 - _ . ! ~ * ' ( )` is escaped.
fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// `decodeURIComponent`; malformed input is kept as written, as the original does.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let byte = value
                .get(i + 1..i + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok());
            let Some(byte) = byte else {
                return value.to_owned();
            };
            decoded.push(byte);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_owned())
}

/// Enabled mods in load order. Every entry carries its position, so the original
/// reproduces this exact order instead of sorting by name.
pub fn serialize(catalog: &Catalog, order: &[usize], enabled: &HashSet<usize>) -> String {
    order
        .iter()
        .filter(|index| enabled.contains(index))
        .filter_map(|&index| catalog.mods.get(index))
        .enumerate()
        .map(|(position, item)| {
            let id = &*item.workshop_id;
            let identifier = if item.source == Source::Workshop && is_workshop_id(id) {
                id.to_owned()
            } else if is_workshop_id(id) {
                format!("{LOCAL_PREFIX}{}:{id}", encode(&item.name))
            } else {
                format!("{LOCAL_PREFIX}{}", encode(&item.name))
            };
            format!("{identifier};{position}")
        })
        .collect::<Vec<_>>()
        .join("|")
}

pub fn parse(text: &str) -> Vec<Shared> {
    text.trim()
        .split('|')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (identifier, order) = match entry.split_once(';') {
                Some((identifier, order)) => (identifier, order.trim().parse().ok()),
                None => (entry, None),
            };
            let Some(local) = identifier.strip_prefix(LOCAL_PREFIX) else {
                return Shared {
                    workshop_id: identifier.trim().to_owned(),
                    name: None,
                    load_order: order,
                };
            };
            let (name, workshop_id) = match local.rsplit_once(':') {
                Some((name, id)) if is_workshop_id(id) => (name, id.to_owned()),
                _ => (local, String::new()),
            };
            Shared {
                workshop_id,
                name: Some(decode(name)),
                load_order: order,
            }
        })
        .collect()
}

/// What a shared list needs before it can be applied.
pub struct Resolution {
    /// A preset with the shared mods in their shared order (original semantics).
    pub preset: Preset,
    /// Workshop items to subscribe to.
    pub missing_workshop: Vec<u64>,
    /// Local mods that are not installed and cannot be downloaded.
    pub missing_local: Vec<String>,
}

/// Match shared entries to installed packs by name first, then by Workshop id.
pub fn resolve(
    shared: &[Shared],
    catalog: &Catalog,
    by_workshop: &HashMap<u64, Vec<usize>>,
    name: String,
) -> Resolution {
    let mut resolution = Resolution {
        preset: Preset {
            name,
            mods: vec![],
            // Version 2 orders pinned entries by load order and the rest by name, as the
            // original does for an imported list.
            version: Some(2),
        },
        missing_workshop: vec![],
        missing_local: vec![],
    };
    for entry in shared {
        let by_name = entry.name.as_ref().and_then(|name| {
            catalog
                .by_name
                .get(&name.to_lowercase())
                .and_then(|indices| indices.first())
        });
        let id = entry.workshop_id.parse::<u64>().ok();
        let by_id = id.and_then(|id| by_workshop.get(&id)?.first());
        match by_name.or(by_id) {
            Some(&index) => resolution.preset.mods.push(Entry {
                name: catalog.mods[index].name.to_string(),
                is_enabled: true,
                load_order: entry.load_order,
            }),
            None => match (id, &entry.name) {
                (Some(id), _) => resolution.missing_workshop.push(id),
                (None, Some(name)) => resolution.missing_local.push(name.clone()),
                (None, None) => {}
            },
        }
    }
    // A list where every mod has a position (as this manager writes it) is an exact
    // order; keep it without pinning every mod against load-order rules.
    let mods = &mut resolution.preset.mods;
    if !mods.is_empty() && mods.iter().all(|entry| entry.load_order.is_some()) {
        mods.sort_by_key(|entry| entry.load_order);
        mods.iter_mut().for_each(|entry| entry.load_order = None);
        resolution.preset.version = Some(crate::preset::LIST_ORDER_VERSION);
    }
    resolution.missing_workshop.sort_unstable();
    resolution.missing_workshop.dedup();
    resolution
}
