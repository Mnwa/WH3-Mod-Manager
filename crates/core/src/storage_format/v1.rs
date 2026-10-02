//! Frozen WHM1 schema: decoded for migration, never written again.
use super::{invalid, records::*};
use crate::{Result, storage::Settings};
use rkyv::with::{InlineAsBox, Map};
use rkyv::{Archive, rancor::Error as ArchiveError};

pub(super) const MAGIC: &[u8; 4] = b"WHM1";

// Only the archived form is read; the native struct defines the frozen layout.
#[expect(dead_code)]
#[derive(Archive)]
struct StateRecord<'a> {
    #[rkyv(with = Map<InlineAsBox>)]
    game: Option<&'a str>,
    roots: Vec<RootRecord<'a>>,
    presets: Vec<PresetRecord<'a>>,
    current: Option<PresetRecord<'a>>,
    metadata: Vec<MetaRecord<'a>>,
}

pub(super) fn decode(payload: &[u8]) -> Result<Settings> {
    let record =
        rkyv::access::<ArchivedStateRecord, ArchiveError>(payload).map_err(|_| invalid())?;
    Ok(Settings {
        game_path: record.game.as_ref().map(|path| path.as_ref().into()),
        roots: decode_roots(&record.roots)?,
        presets: record.presets.iter().map(decode_preset).collect(),
        current: record.current.as_ref().map(decode_preset),
        metadata: decode_meta(&record.metadata),
        ..Settings::default()
    })
}
