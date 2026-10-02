//! Record types shared by every WHM schema version. Changing any of them changes
//! the archived layout of all versions, so they are frozen like WHM1 itself.
use super::invalid;
use crate::{
    Error, Result,
    catalog::Source,
    metadata::Metadata,
    preset::{Entry, Preset},
};
use rkyv::with::{InlineAsBox, Map};
use rkyv::{Archive, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Archive, Serialize)]
pub(super) struct RootRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    pub path: &'a str,
    pub source: u8,
}

#[derive(Archive, Serialize)]
pub(super) struct PresetRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    pub name: &'a str,
    pub version: Option<u32>,
    pub mods: Vec<EntryRecord<'a>>,
}

#[derive(Archive, Serialize)]
pub(super) struct EntryRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    pub name: &'a str,
    pub enabled: bool,
    pub order: Option<u32>,
}

#[derive(Archive, Serialize)]
pub(super) struct MetaRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    pub name: &'a str,
    #[rkyv(with = InlineAsBox)]
    pub title: &'a str,
    #[rkyv(with = InlineAsBox)]
    pub author: &'a str,
    #[rkyv(with = InlineAsBox)]
    pub workshop: &'a str,
    #[rkyv(with = Map<InlineAsBox>)]
    pub categories: Vec<&'a str>,
    #[rkyv(with = Map<InlineAsBox>)]
    pub tags: Vec<&'a str>,
    pub required: Vec<RequiredRecord<'a>>,
}

#[derive(Archive, Serialize)]
pub(super) struct RequiredRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    pub id: &'a str,
    #[rkyv(with = InlineAsBox)]
    pub name: &'a str,
}

pub(super) fn path_str(path: &std::path::Path) -> Result<&str> {
    path.to_str().ok_or_else(|| {
        Error::Invalid(crate::message!(
            "Path contains invalid Unicode",
            "Путь содержит невалидный Unicode"
        ))
    })
}

pub(super) fn root_records(roots: &[(PathBuf, Source)]) -> Result<Vec<RootRecord<'_>>> {
    roots
        .iter()
        .map(|(path, source)| {
            Ok(RootRecord {
                path: path_str(path)?,
                source: match source {
                    Source::Data => 0,
                    Source::Workshop => 1,
                    Source::Custom => 2,
                },
            })
        })
        .collect()
}

pub(super) fn decode_roots(records: &[ArchivedRootRecord]) -> Result<Vec<(PathBuf, Source)>> {
    records
        .iter()
        .map(|root| {
            Ok((
                PathBuf::from(root.path.as_ref()),
                match root.source {
                    0 => Source::Data,
                    1 => Source::Workshop,
                    2 => Source::Custom,
                    _ => return Err(invalid()),
                },
            ))
        })
        .collect()
}

pub(super) fn preset_record(preset: &Preset) -> Result<PresetRecord<'_>> {
    Ok(PresetRecord {
        name: &preset.name,
        version: preset.version,
        mods: preset
            .mods
            .iter()
            .map(|entry| {
                Ok(EntryRecord {
                    name: &entry.name,
                    enabled: entry.is_enabled,
                    order: entry
                        .load_order
                        .map(u32::try_from)
                        .transpose()
                        .map_err(|_| invalid())?,
                })
            })
            .collect::<Result<_>>()?,
    })
}

pub(super) fn decode_preset(record: &ArchivedPresetRecord) -> Preset {
    Preset {
        name: record.name.to_string(),
        version: record.version.as_ref().map(|v| v.to_native()),
        mods: record
            .mods
            .iter()
            .map(|entry| Entry {
                name: entry.name.to_string(),
                is_enabled: entry.enabled,
                load_order: entry.order.as_ref().map(|n| n.to_native() as usize),
            })
            .collect(),
    }
}

pub(super) fn meta_records(metadata: &BTreeMap<String, Metadata>) -> Vec<MetaRecord<'_>> {
    metadata
        .iter()
        .map(|(name, meta)| MetaRecord {
            name,
            title: &meta.human_name,
            author: &meta.author,
            workshop: &meta.workshop_id,
            categories: meta.categories.iter().map(String::as_str).collect(),
            tags: meta.tags.iter().map(String::as_str).collect(),
            required: meta
                .req_mod_id_to_name
                .iter()
                .map(|(id, name)| RequiredRecord { id, name })
                .collect(),
        })
        .collect()
}

pub(super) fn decode_meta(records: &[ArchivedMetaRecord]) -> BTreeMap<String, Metadata> {
    records
        .iter()
        .map(|meta| {
            (
                meta.name.to_string(),
                Metadata {
                    human_name: meta.title.to_string(),
                    author: meta.author.to_string(),
                    workshop_id: meta.workshop.to_string(),
                    categories: meta.categories.iter().map(|s| s.to_string()).collect(),
                    tags: meta.tags.iter().map(|s| s.to_string()).collect(),
                    req_mod_id_to_name: meta
                        .required
                        .iter()
                        .map(|r| (r.id.to_string(), r.name.to_string()))
                        .collect(),
                },
            )
        })
        .collect()
}
