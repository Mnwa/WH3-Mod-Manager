//! WHM framing: magic(4) | payload_len(u32 LE) | CRC32(u32 LE) | rkyv record.
//! Schemas are separate from domain types. WHM1 is frozen and only decoded;
//! every save writes the current version, which migrates older libraries.
mod records;
mod v1;
mod v2;
mod v3;

use crate::{Error, Result, storage::Settings};
use rkyv::rancor::Error as ArchiveError;

pub(crate) const MAX_FILE: u64 = 128 * 1024 * 1024;
const HEADER: usize = 12;

fn invalid() -> Error {
    Error::Invalid(crate::message!(
        "Corrupt or unsupported WHM binary library",
        "Повреждённое или неподдерживаемое бинарное хранилище WHM"
    ))
}

/// Serialize `record` directly after a zeroed header, then fill in the frame.
fn frame<T>(magic: &[u8; 4], record: &T) -> Result<Vec<u8>>
where
    T: for<'a> rkyv::Serialize<
            rkyv::api::high::HighSerializer<
                Vec<u8>,
                rkyv::ser::allocator::ArenaHandle<'a>,
                ArchiveError,
            >,
        >,
{
    let mut bytes = rkyv::api::high::to_bytes_in::<_, ArchiveError>(record, vec![0; HEADER])
        .map_err(|_| invalid())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(invalid());
    }
    let size = u32::try_from(bytes.len() - HEADER).map_err(|_| invalid())?;
    let checksum = crc32fast::hash(&bytes[HEADER..]);
    bytes[..4].copy_from_slice(magic);
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes[8..12].copy_from_slice(&checksum.to_le_bytes());
    Ok(bytes)
}

pub(crate) fn encode(settings: &Settings) -> Result<Vec<u8>> {
    v3::encode(settings)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Settings> {
    if bytes.len() < HEADER || bytes.len() as u64 > MAX_FILE {
        return Err(invalid());
    }
    let length = u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| invalid())?) as usize;
    let checksum = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| invalid())?);
    let payload = &bytes[HEADER..];
    if length != payload.len() || crc32fast::hash(payload) != checksum {
        return Err(invalid());
    }
    let magic = &bytes[..4];
    if magic == v3::MAGIC {
        v3::decode(payload)
    } else if magic == v2::MAGIC {
        v2::decode(payload)
    } else if magic == v1::MAGIC {
        v1::decode(payload)
    } else {
        Err(invalid())
    }
}
