//! PFH4/PFH5 pack headers, indexes and packed file payloads.
mod compression;
mod read;

use crate::{Error, Result, error::io};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub use crate::pack_writer::{encode, write};
pub use compression::{MAX_ENTRY_SIZE, decompress};
pub use read::{Reader, read_file, read_files};

const MAX_INDEX: u64 = 128 * 1024 * 1024;

#[derive(Debug)]
pub struct Header {
    pub version: u8,
    pub flags: u32,
    pub dependencies: Vec<String>,
    pub file_count: u32,
    pub index_size: u32,
    pub index_start: u64,
    pub pack_size: u64,
}

#[derive(Debug, Clone)]
pub struct PackedFile {
    pub name: String,
    pub size: u32,
    pub offset: u64,
    pub compressed: bool,
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::Pack(crate::message!("truncated integer", "обрезанное число")))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub fn header(path: &Path) -> Result<Header> {
    let mut file = File::open(path).map_err(|e| io(path, e))?;
    read_header(&mut file, path)
}

/// Reads the header from the start of an open pack; `path` is only used for error messages.
fn read_header(file: &mut File, path: &Path) -> Result<Header> {
    let pack_size = file.metadata().map_err(|e| io(path, e))?.len();
    let mut bytes = [0; 28];
    file.read_exact(&mut bytes).map_err(|e| io(path, e))?;
    let version = match &bytes[..4] {
        b"PFH5" => 5,
        b"PFH4" => 4,
        _ => {
            return Err(Error::Pack(crate::message!(
                "{}: only PFH4/PFH5 are supported",
                "{}: поддерживаются PFH4/PFH5",
                path.display()
            )));
        }
    };
    let dependency_count = word(&bytes, 8)?;
    let dependency_size = word(&bytes, 12)? as u64;
    let index_size = word(&bytes, 20)?;
    if dependency_size + u64::from(index_size) > MAX_INDEX
        || 28 + dependency_size + u64::from(index_size) > pack_size
    {
        return Err(Error::Pack(crate::message!(
            "index size is out of bounds",
            "размер индекса вне допустимых границ"
        )));
    }
    let mut dependencies = vec![0; dependency_size as usize];
    file.read_exact(&mut dependencies)
        .map_err(|e| io(path, e))?;
    let mut names = Vec::new();
    let mut remaining = dependencies.as_slice();
    for _ in 0..dependency_count {
        let end = remaining.iter().position(|&b| b == 0).ok_or_else(|| {
            Error::Pack(crate::message!(
                "truncated dependency",
                "обрезанная зависимость"
            ))
        })?;
        names.push(String::from_utf8_lossy(&remaining[..end]).into_owned());
        remaining = &remaining[end + 1..];
    }
    if !remaining.is_empty() {
        return Err(Error::Pack(crate::message!(
            "trailing dependency data",
            "лишние данные в зависимостях"
        )));
    }
    Ok(Header {
        version,
        flags: word(&bytes, 4)?,
        dependencies: names,
        file_count: word(&bytes, 16)?,
        index_size,
        index_start: 28 + dependency_size,
        pack_size,
    })
}

pub fn index(path: &Path) -> Result<Vec<PackedFile>> {
    let mut file = File::open(path).map_err(|e| io(path, e))?;
    read_index(&mut file, path)
}

/// Reads the header and index of an open pack, leaving the cursor after the index.
fn read_index(file: &mut File, path: &Path) -> Result<Vec<PackedFile>> {
    let header = read_header(file, path)?;
    file.seek(SeekFrom::Start(header.index_start))
        .map_err(|e| io(path, e))?;
    let mut bytes = vec![0; header.index_size as usize];
    file.read_exact(&mut bytes).map_err(|e| io(path, e))?;
    parse_index(&header, &bytes)
}

pub fn parse_index(header: &Header, bytes: &[u8]) -> Result<Vec<PackedFile>> {
    if bytes.len() != header.index_size as usize || header.file_count as usize > bytes.len() / 5 {
        return Err(Error::Pack(crate::message!(
            "invalid entry count",
            "неверное количество записей"
        )));
    }
    let mut entries = Vec::with_capacity(header.file_count as usize);
    let mut position = 0;
    let mut offset = header.index_start + u64::from(header.index_size);
    for _ in 0..header.file_count {
        let size = word(bytes, position)?;
        position += 4;
        if header.version == 5 && header.flags & 0x40 != 0 {
            word(bytes, position)?;
            position += 4;
        }
        // PFH4 stores a timestamp when bit 0x40 is set.
        if header.version == 4 && header.flags & 0x40 != 0 {
            word(bytes, position)?;
            position += 4;
        }
        let compressed = if header.version == 5 {
            let flag = *bytes.get(position).ok_or_else(|| {
                Error::Pack(crate::message!(
                    "missing compression flag",
                    "нет флага сжатия"
                ))
            })?;
            position += 1;
            if flag > 1 {
                return Err(Error::Pack(crate::message!(
                    "invalid compression flag",
                    "неверный флаг сжатия"
                )));
            }
            flag == 1
        } else {
            false
        };
        let rest = bytes
            .get(position..)
            .ok_or_else(|| Error::Pack(crate::message!("truncated name", "обрезанное имя")))?;
        let end = rest.iter().position(|&b| b == 0).ok_or_else(|| {
            Error::Pack(crate::message!("unterminated name", "незавершённое имя"))
        })?;
        let name = String::from_utf8_lossy(&rest[..end]).into_owned();
        position += end + 1;
        let next = offset
            .checked_add(u64::from(size))
            .ok_or_else(|| Error::Pack(crate::message!("size overflow", "переполнение размера")))?;
        if next > header.pack_size {
            return Err(Error::Pack(crate::message!(
                "file extends beyond pack bounds",
                "файл выходит за границы pack"
            )));
        }
        entries.push(PackedFile {
            name,
            size,
            offset,
            compressed,
        });
        offset = next;
    }
    if position != bytes.len() {
        return Err(Error::Pack(crate::message!(
            "trailing index data",
            "лишние данные в индексе"
        )));
    }
    Ok(entries)
}
