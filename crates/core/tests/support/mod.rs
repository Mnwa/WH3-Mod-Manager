//! Pack, compression and DB table builders for the pack and DB tests.
#![allow(dead_code, clippy::unwrap_used)]
use std::{fs, path::Path};

/// One packed file: name, stored bytes and the PFH5 compression flag.
pub struct Entry<'a> {
    pub name: &'a str,
    pub stored: Vec<u8>,
    pub compressed: bool,
}

pub fn plain<'a>(name: &'a str, bytes: &[u8]) -> Entry<'a> {
    Entry {
        name,
        stored: bytes.to_vec(),
        compressed: false,
    }
}

pub fn packed<'a>(name: &'a str, stored: Vec<u8>) -> Entry<'a> {
    Entry {
        name,
        stored,
        compressed: true,
    }
}

/// PFH5 (with compression flags) or PFH4 (without), optionally with per-file timestamps.
pub fn pack_bytes(pfh4: bool, timestamps: bool, entries: &[Entry]) -> Vec<u8> {
    let mut index = Vec::new();
    for entry in entries {
        index.extend_from_slice(&(entry.stored.len() as u32).to_le_bytes());
        if timestamps {
            index.extend_from_slice(&0x5eed_u32.to_le_bytes());
        }
        if !pfh4 {
            index.push(u8::from(entry.compressed));
        }
        index.extend_from_slice(entry.name.as_bytes());
        index.push(0);
    }
    let mut bytes = if pfh4 { b"PFH4" } else { b"PFH5" }.to_vec();
    let flags = 3 | if timestamps { 0x40 } else { 0 };
    for word in [flags, 0, 0, entries.len() as u32, index.len() as u32, 0] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend(index);
    for entry in entries {
        bytes.extend_from_slice(&entry.stored);
    }
    bytes
}

pub fn write_pack(path: &Path, entries: &[Entry]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, pack_bytes(false, false, entries)).unwrap();
}

fn sized(bytes: &[u8], stream: Vec<u8>) -> Vec<u8> {
    let mut stored = (bytes.len() as u32).to_le_bytes().to_vec();
    stored.extend(stream);
    stored
}

pub fn zstd(bytes: &[u8]) -> Vec<u8> {
    let frame =
        ruzstd::encoding::compress_to_vec(bytes, ruzstd::encoding::CompressionLevel::Fastest);
    sized(bytes, frame)
}

pub fn lz4(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = lz4_flex::frame::FrameEncoder::new(Vec::new());
    encoder.write_all(bytes).unwrap();
    sized(bytes, encoder.finish().unwrap())
}

/// CA's layout: the size, the 5 LZMA property bytes, then the raw stream.
pub fn lzma(bytes: &[u8]) -> Vec<u8> {
    let options = lzma_rs::compress::Options {
        unpacked_size: lzma_rs::compress::UnpackedSize::SkipWritingToHeader,
    };
    let mut stream = Vec::new();
    lzma_rs::lzma_compress_with_options(&mut &bytes[..], &mut stream, &options).unwrap();
    sized(bytes, stream)
}

/// A cell written in the binary DB layout of its field type.
#[derive(Clone, Copy)]
pub enum Cell<'a> {
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Colour(u32),
    Text(&'a str),
    Wide(&'a str),
    Optional(Option<&'a str>),
}

impl Cell<'_> {
    fn write(self, bytes: &mut Vec<u8>) {
        match self {
            Self::Bool(value) => bytes.push(u8::from(value)),
            Self::I16(value) => bytes.extend_from_slice(&value.to_le_bytes()),
            Self::I32(value) => bytes.extend_from_slice(&value.to_le_bytes()),
            Self::I64(value) => bytes.extend_from_slice(&value.to_le_bytes()),
            Self::F32(value) => bytes.extend_from_slice(&value.to_le_bytes()),
            Self::F64(value) => bytes.extend_from_slice(&value.to_le_bytes()),
            Self::Colour(value) => bytes.extend_from_slice(&value.to_le_bytes()),
            Self::Text(text) => {
                bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
            }
            Self::Wide(text) => {
                let units: Vec<u16> = text.encode_utf16().collect();
                bytes.extend_from_slice(&(units.len() as u16).to_le_bytes());
                for unit in units {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            }
            Self::Optional(None) => bytes.push(0),
            Self::Optional(Some(text)) => {
                bytes.push(1);
                Self::Text(text).write(bytes);
            }
        }
    }
}

/// Encodes rows only, for appending after a header.
pub fn rows_bytes(rows: &[Vec<Cell>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for row in rows {
        for &cell in row {
            cell.write(&mut bytes);
        }
    }
    bytes
}

/// A full DB table file with an optional GUID and version marker.
pub fn table(guid: Option<&str>, version: Option<i32>, rows: &[Vec<Cell>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(guid) = guid {
        bytes.extend_from_slice(&[0xfd, 0xfe, 0xfc, 0xff]);
        Cell::Wide(guid).write(&mut bytes);
    }
    if let Some(version) = version {
        bytes.extend_from_slice(&[0xfc, 0xfd, 0xfe, 0xff]);
        bytes.extend_from_slice(&version.to_le_bytes());
    }
    bytes.push(1);
    bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    bytes.extend(rows_bytes(rows));
    bytes
}

/// A `units_custom_battle_permissions_tables` version 11 row.
pub fn permission<'a>(faction: &'a str, unit: &'a str, general: bool) -> Vec<Cell<'a>> {
    vec![
        Cell::Text(faction),
        Cell::Bool(general),
        Cell::Text(unit),
        Cell::Bool(true),
        Cell::Bool(false),
        Cell::Optional(general.then_some("portrait")),
        Cell::Optional(None),
        Cell::Optional(None),
        Cell::Bool(false),
        Cell::Optional(None),
        Cell::Bool(true),
    ]
}
