//! Frozen WHM2 schema (hidden/always-enabled mods, start options): decoded for migration only.
use super::{invalid, records::*};
use crate::{
    Result,
    storage::{GameOptions, Settings},
};
use rkyv::with::{InlineAsBox, Map};
use rkyv::{Archive, rancor::Error as ArchiveError};

pub(super) const MAGIC: &[u8; 4] = b"WHM2";

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
    #[rkyv(with = Map<InlineAsBox>)]
    hidden: Vec<&'a str>,
    #[rkyv(with = Map<InlineAsBox>)]
    always_enabled: Vec<&'a str>,
    /// Bit set of [`GameOptions`]; unknown bits are rejected on load.
    options: u32,
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
        hidden: record.hidden.iter().map(|s| s.to_string()).collect(),
        always_enabled: record
            .always_enabled
            .iter()
            .map(|s| s.to_string())
            .collect(),
        options: GameOptions::from_bits(record.options.to_native()).ok_or_else(invalid)?,
        ..Settings::default()
    })
}
