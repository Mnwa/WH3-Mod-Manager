//! Packed file payloads, read through a pack index.
use super::{MAX_ENTRY_SIZE, PackedFile, compression, read_index};
use crate::{Error, Result, error::io};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// An open pack for reading several entries of one index without reopening the file.
#[derive(Debug)]
pub struct Reader {
    file: File,
    path: PathBuf,
    /// Length at open time. A stat per entry is a measurable cost on slow file systems (WSL,
    /// network shares); a pack shortened later is still caught by the failing read.
    len: u64,
}

impl Reader {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| io(path, e))?;
        let len = file.metadata().map_err(|e| io(path, e))?.len();
        Ok(Self {
            file,
            path: path.to_owned(),
            len,
        })
    }

    /// Opens a pack and reads its index with the same handle.
    pub fn open_with_index(path: &Path) -> Result<(Self, Vec<PackedFile>)> {
        let mut reader = Self::open(path)?;
        let entries = read_index(&mut reader.file, path)?;
        Ok((reader, entries))
    }

    /// Reads one entry and decompresses it if needed.
    ///
    /// `entry` should come from this pack's index. Its bounds are checked again because the
    /// index may be stale: the pack can be replaced or shortened while the manager runs.
    pub fn read(&mut self, entry: &PackedFile) -> Result<Vec<u8>> {
        let beyond = || {
            Error::Pack(crate::message!(
                "{}: file extends beyond pack bounds",
                "{}: файл выходит за границы pack",
                entry.name
            ))
        };
        let end = entry.offset.checked_add(u64::from(entry.size));
        if end.is_none_or(|end| end > self.len) {
            return Err(beyond());
        }
        if u64::from(entry.size) > MAX_ENTRY_SIZE {
            return Err(Error::Pack(crate::message!(
                "{}: file is too large: {} bytes",
                "{}: файл слишком большой: {} байт",
                entry.name,
                entry.size
            )));
        }
        self.file
            .seek(SeekFrom::Start(entry.offset))
            .map_err(|e| io(&self.path, e))?;
        let mut stored = vec![0; entry.size as usize];
        self.file.read_exact(&mut stored).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                beyond()
            } else {
                io(&self.path, e)
            }
        })?;
        if entry.compressed {
            compression::decompress(&stored).map_err(|e| {
                Error::Pack(crate::message!("{}: ", "{}: ", entry.name).concat(e.message()))
            })
        } else {
            Ok(stored)
        }
    }
}

/// Packed paths are matched like the game does: case-insensitively, with either separator.
fn normalize(name: &str) -> String {
    name.replace('/', "\\").to_lowercase()
}

/// Reads one packed file by name.
pub fn read_file(path: &Path, name: &str) -> Result<Vec<u8>> {
    read_files(path, &[name])?.pop().flatten().ok_or_else(|| {
        Error::Pack(crate::message!(
            "{}: file not found in pack",
            "{}: файл не найден в pack",
            name
        ))
    })
}

/// Reads several packed files with one open, in the order of `names`.
///
/// A name the pack does not contain yields `None`. When a name occurs more than once in the
/// index, the first entry is read, as in the compatibility overlay.
pub fn read_files(path: &Path, names: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
    let (mut reader, entries) = Reader::open_with_index(path)?;
    let mut by_name: HashMap<String, &PackedFile> = HashMap::with_capacity(entries.len());
    for entry in &entries {
        by_name.entry(normalize(&entry.name)).or_insert(entry);
    }
    names
        .iter()
        .map(|name| {
            by_name
                .get(&normalize(name))
                .map(|entry| reader.read(entry))
                .transpose()
        })
        .collect()
}
