//! WHM1: magic(4) | payload_len(u32 LE) | CRC32(u32 LE) | rkyv record.
//! Схема отделена от доменных типов: её изменение требует новой версии и миграции.
use crate::{
    Error, Result,
    catalog::Source,
    metadata::Metadata,
    preset::{Entry, Preset},
    storage::Settings,
};
use rkyv::with::{InlineAsBox, Map};
use rkyv::{Archive, Serialize, rancor::Error as ArchiveError};

const MAGIC: &[u8; 4] = b"WHM1";
pub(crate) const MAX_FILE: u64 = 128 * 1024 * 1024;

#[derive(Archive, Serialize)]
struct StateRecord<'a> {
    #[rkyv(with = Map<InlineAsBox>)]
    game: Option<&'a str>,
    roots: Vec<RootRecord<'a>>,
    presets: Vec<PresetRecord<'a>>,
    current: Option<PresetRecord<'a>>,
    metadata: Vec<MetaRecord<'a>>,
}

#[derive(Archive, Serialize)]
struct RootRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    path: &'a str,
    source: u8,
}

#[derive(Archive, Serialize)]
struct PresetRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    name: &'a str,
    version: Option<u32>,
    mods: Vec<EntryRecord<'a>>,
}

#[derive(Archive, Serialize)]
struct EntryRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    name: &'a str,
    enabled: bool,
    order: Option<u32>,
}

#[derive(Archive, Serialize)]
struct MetaRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    name: &'a str,
    #[rkyv(with = InlineAsBox)]
    title: &'a str,
    #[rkyv(with = InlineAsBox)]
    author: &'a str,
    #[rkyv(with = InlineAsBox)]
    workshop: &'a str,
    #[rkyv(with = Map<InlineAsBox>)]
    categories: Vec<&'a str>,
    #[rkyv(with = Map<InlineAsBox>)]
    tags: Vec<&'a str>,
    required: Vec<RequiredRecord<'a>>,
}

#[derive(Archive, Serialize)]
struct RequiredRecord<'a> {
    #[rkyv(with = InlineAsBox)]
    id: &'a str,
    #[rkyv(with = InlineAsBox)]
    name: &'a str,
}

fn invalid() -> Error {
    Error::Invalid("Повреждённое или неподдерживаемое бинарное хранилище WHM1".into())
}

fn preset_record(preset: &Preset) -> Result<PresetRecord<'_>> {
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

pub(crate) fn encode(settings: &Settings) -> Result<Vec<u8>> {
    fn path_str(path: &std::path::Path) -> Result<&str> {
        path.to_str()
            .ok_or_else(|| Error::Invalid("Путь содержит невалидный Unicode".into()))
    }
    let record = StateRecord {
        game: settings.game_path.as_deref().map(path_str).transpose()?,
        roots: settings
            .roots
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
            .collect::<Result<_>>()?,
        presets: settings
            .presets
            .iter()
            .map(preset_record)
            .collect::<Result<_>>()?,
        current: settings.current.as_ref().map(preset_record).transpose()?,
        metadata: settings
            .metadata
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
            .collect(),
    };
    // Строки заимствуются; сериализатор пишет прямо после заголовка без промежуточного JSON.
    let mut bytes = rkyv::api::high::to_bytes_in::<_, ArchiveError>(&record, vec![0; 12])
        .map_err(|_| invalid())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(invalid());
    }
    let size = u32::try_from(bytes.len() - 12).map_err(|_| invalid())?;
    let checksum = crc32fast::hash(&bytes[12..]);
    bytes[..4].copy_from_slice(MAGIC);
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes[8..12].copy_from_slice(&checksum.to_le_bytes());
    Ok(bytes)
}

fn decode_preset(record: &ArchivedPresetRecord) -> Preset {
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

pub(crate) fn decode(bytes: &[u8]) -> Result<Settings> {
    if bytes.len() < 12 || bytes.len() as u64 > MAX_FILE || &bytes[..4] != MAGIC {
        return Err(invalid());
    }
    let length = u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| invalid())?) as usize;
    let checksum = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| invalid())?);
    let payload = &bytes[12..];
    if length != payload.len() || crc32fast::hash(payload) != checksum {
        return Err(invalid());
    }
    let record =
        rkyv::access::<ArchivedStateRecord, ArchiveError>(payload).map_err(|_| invalid())?;
    Ok(Settings {
        game_path: record
            .game
            .as_ref()
            .map(|path| std::path::PathBuf::from(path.as_ref())),
        roots: record
            .roots
            .iter()
            .map(|root| {
                Ok((
                    std::path::PathBuf::from(root.path.as_ref()),
                    match root.source {
                        0 => Source::Data,
                        1 => Source::Workshop,
                        2 => Source::Custom,
                        _ => return Err(invalid()),
                    },
                ))
            })
            .collect::<Result<_>>()?,
        presets: record.presets.iter().map(decode_preset).collect(),
        current: record.current.as_ref().map(decode_preset),
        metadata: record
            .metadata
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
            .collect(),
    })
}
