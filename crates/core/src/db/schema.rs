//! The embedded Warhammer III DB schema.
//!
//! `assets/schema_wh3.json.zst` is the original manager's bundled schema, copied unchanged so it
//! can be refreshed by copying a newer file. Its table definitions derive from the RPFM schemas
//! (<https://github.com/Frodo45127/rpfm-schemas>, MIT, Copyright (c) 2020 Ismael Gutiérrez
//! González); origin and checksum are recorded in `assets/NOTICE`. It is decoded once, on first
//! use: most sessions never read DB tables, and the ~9 MB of JSON costs ~25 ms (release build)
//! plus the memory of the parsed definitions.
use crate::{Error, Result, localization::Message};
use serde::Deserialize;
use std::{collections::HashMap, io::Read, sync::OnceLock};

const SCHEMA: &[u8] = include_bytes!("../../assets/schema_wh3.json.zst");
/// Upper bound for the decompressed schema; the current file is about 9 MB.
const MAX_SCHEMA_SIZE: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Boolean,
    I16,
    I32,
    I64,
    F32,
    F64,
    /// Four bytes; read as a little-endian `u32`.
    ColourRgb,
    /// `u16` byte length, then UTF-8.
    StringU8,
    /// `u16` length in UTF-16 code units, then UTF-16LE.
    StringU16,
    /// A `0`/`1` presence byte, then the value when present.
    OptionalStringU8,
    OptionalStringU16,
    OptionalI16,
    OptionalI32,
    OptionalI64,
    /// Nested sequences and types this reader does not know; tables using them are not read.
    Unsupported,
}

impl FieldType {
    fn parse(value: &serde_json::Value) -> Self {
        match value.as_str() {
            Some("Boolean") => Self::Boolean,
            Some("I16") => Self::I16,
            Some("I32") => Self::I32,
            Some("I64") => Self::I64,
            Some("F32") => Self::F32,
            Some("F64") => Self::F64,
            Some("ColourRGB") => Self::ColourRgb,
            Some("StringU8") => Self::StringU8,
            Some("StringU16") => Self::StringU16,
            Some("OptionalStringU8") => Self::OptionalStringU8,
            Some("OptionalStringU16") => Self::OptionalStringU16,
            Some("OptionalI16") => Self::OptionalI16,
            Some("OptionalI32") => Self::OptionalI32,
            Some("OptionalI64") => Self::OptionalI64,
            _ => Self::Unsupported,
        }
    }

    /// Fewest bytes a value can occupy; bounds row counts before anything is allocated.
    pub(super) fn min_size(self) -> usize {
        match self {
            Self::Boolean
            | Self::OptionalStringU8
            | Self::OptionalStringU16
            | Self::OptionalI16
            | Self::OptionalI32
            | Self::OptionalI64
            | Self::Unsupported => 1,
            Self::I16 | Self::StringU8 | Self::StringU16 => 2,
            Self::I32 | Self::F32 | Self::ColourRgb => 4,
            Self::I64 | Self::F64 => 8,
        }
    }
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub kind: FieldType,
    /// Part of the row key: the game keeps one row per key across all files of a table.
    pub is_key: bool,
}

/// The binary layout of one table version.
#[derive(Debug)]
pub struct Definition {
    pub version: i32,
    /// In binary order.
    pub fields: Vec<Field>,
}

impl Definition {
    /// Positions of the key fields in binary order.
    pub fn key_fields(&self) -> impl Iterator<Item = usize> + '_ {
        self.fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.is_key)
            .map(|(index, _)| index)
    }

    pub fn field(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|field| field.name == name)
    }
}

#[derive(Debug)]
pub struct Schema {
    tables: HashMap<String, Vec<Definition>>,
}

impl Schema {
    /// Every known version of a table, e.g. `main_units_tables`.
    pub fn versions(&self, table: &str) -> Option<&[Definition]> {
        self.tables.get(table).map(Vec::as_slice)
    }

    /// The definition for a file's version; a file without a version marker is version 0.
    ///
    /// The original manager (`resolveParsedDBVersion`, `utility/packFileHelpers.ts:172`) accepts
    /// only an exact match or, for an unversioned file, the version 0 definition.
    pub fn resolve(&self, table: &str, version: Option<i32>) -> Option<&Definition> {
        let version = version.unwrap_or(0);
        self.versions(table)?
            .iter()
            .find(|definition| definition.version == version)
    }

    /// Parses an uncompressed schema in the RPFM JSON format.
    pub fn from_json(json: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct RawSchema {
            definitions: HashMap<String, Vec<RawDefinition>>,
        }
        #[derive(Deserialize)]
        struct RawDefinition {
            version: i32,
            fields: Vec<RawField>,
        }
        #[derive(Deserialize)]
        struct RawField {
            name: String,
            field_type: serde_json::Value,
            is_key: bool,
        }
        let raw: RawSchema = serde_json::from_slice(json)?;
        let tables = raw
            .definitions
            .into_iter()
            .map(|(table, versions)| {
                let versions = versions
                    .into_iter()
                    .map(|definition| Definition {
                        version: definition.version,
                        fields: definition
                            .fields
                            .into_iter()
                            .map(|field| Field {
                                kind: FieldType::parse(&field.field_type),
                                name: field.name,
                                is_key: field.is_key,
                            })
                            .collect(),
                    })
                    .collect();
                (table, versions)
            })
            .collect();
        Ok(Self { tables })
    }
}

/// Failures keep only the message: the cached result must be shareable between threads.
fn load() -> std::result::Result<Schema, Message> {
    let failed = |detail: &dyn std::fmt::Display| {
        crate::message!("embedded DB schema: {}", "встроенная схема DB: {}", detail)
    };
    let decoder = ruzstd::decoding::StreamingDecoder::new(SCHEMA).map_err(|e| failed(&e))?;
    let mut json = Vec::new();
    decoder
        .take(MAX_SCHEMA_SIZE)
        .read_to_end(&mut json)
        .map_err(|e| failed(&e))?;
    Schema::from_json(&json).map_err(|e| failed(&e))
}

/// The embedded schema, decoded on first use and shared afterwards.
pub fn schema() -> Result<&'static Schema> {
    static CACHE: OnceLock<std::result::Result<Schema, Message>> = OnceLock::new();
    CACHE
        .get_or_init(load)
        .as_ref()
        .map_err(|message| Error::Invalid(message.clone()))
}
