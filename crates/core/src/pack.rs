use crate::{Error, Result, error::io};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

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

#[derive(Debug)]
pub struct PackedFile {
    pub name: String,
    pub size: u32,
    pub offset: u64,
    pub compressed: bool,
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::Pack("обрезанное число".into()))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub fn header(path: &Path) -> Result<Header> {
    let mut file = File::open(path).map_err(|e| io(path, e))?;
    let pack_size = file.metadata().map_err(|e| io(path, e))?.len();
    let mut bytes = [0; 28];
    file.read_exact(&mut bytes).map_err(|e| io(path, e))?;
    let version = match &bytes[..4] {
        b"PFH5" => 5,
        b"PFH4" => 4,
        _ => {
            return Err(Error::Pack(format!(
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
        return Err(Error::Pack("размер индекса вне допустимых границ".into()));
    }
    let mut dependencies = vec![0; dependency_size as usize];
    file.read_exact(&mut dependencies)
        .map_err(|e| io(path, e))?;
    let mut names = Vec::new();
    let mut remaining = dependencies.as_slice();
    for _ in 0..dependency_count {
        let end = remaining
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| Error::Pack("обрезанная зависимость".into()))?;
        names.push(String::from_utf8_lossy(&remaining[..end]).into_owned());
        remaining = &remaining[end + 1..];
    }
    if !remaining.is_empty() {
        return Err(Error::Pack("лишние данные в зависимостях".into()));
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
    let header = header(path)?;
    let mut file = File::open(path).map_err(|e| io(path, e))?;
    file.seek(SeekFrom::Start(header.index_start))
        .map_err(|e| io(path, e))?;
    let mut bytes = vec![0; header.index_size as usize];
    file.read_exact(&mut bytes).map_err(|e| io(path, e))?;
    parse_index(&header, &bytes)
}

pub fn parse_index(header: &Header, bytes: &[u8]) -> Result<Vec<PackedFile>> {
    if bytes.len() != header.index_size as usize || header.file_count as usize > bytes.len() / 5 {
        return Err(Error::Pack("неверное количество записей".into()));
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
        // PFH4 хранит timestamp при установленном бите 0x40.
        if header.version == 4 && header.flags & 0x40 != 0 {
            word(bytes, position)?;
            position += 4;
        }
        let compressed = if header.version == 5 {
            let flag = *bytes
                .get(position)
                .ok_or_else(|| Error::Pack("нет флага сжатия".into()))?;
            position += 1;
            if flag > 1 {
                return Err(Error::Pack("неверный флаг сжатия".into()));
            }
            flag == 1
        } else {
            false
        };
        let rest = bytes
            .get(position..)
            .ok_or_else(|| Error::Pack("обрезанное имя".into()))?;
        let end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| Error::Pack("незавершённое имя".into()))?;
        let name = String::from_utf8_lossy(&rest[..end]).into_owned();
        position += end + 1;
        let next = offset
            .checked_add(u64::from(size))
            .ok_or_else(|| Error::Pack("переполнение размера".into()))?;
        if next > header.pack_size {
            return Err(Error::Pack("файл выходит за границы pack".into()));
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
        return Err(Error::Pack("лишние данные в индексе".into()));
    }
    Ok(entries)
}
