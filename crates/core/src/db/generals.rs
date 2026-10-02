//! "Make units generals": an extra custom battle permissions table that lets every unit lead an
//! army, ported from the original manager's `createBattlePermissionsData`
//! (`packFileSerializer.ts:597`) and its writer in `writeStartGamePack` (`:2358`).
use super::{
    header::{GUID_MARKER, VERSION_MARKER},
    read_header,
    rows::Rows,
    schema, unknown_version,
};
use crate::{Error, Result, catalog::Catalog, pack};
use std::{
    collections::HashSet,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const TABLE: &str = "units_custom_battle_permissions_tables";
/// Sorts before other DB file names, so its rows win key collisions in the table.
pub const UNITS_GENERALS_PATH: &str = "db\\units_custom_battle_permissions_tables\\!!!!whmm_out";
/// The fixed GUID the original manager writes, kept so both managers produce the same file.
const GUID: &str = "129d32d8-3563-4d4f-8e19-a815e834e456";
const GENERAL_FIELD: &str = "general_unit";

/// Rows collected from the source files, already transformed.
struct Output {
    rows: u32,
    data: Vec<u8>,
}

/// Encodes the generated table from the enabled mods and the vanilla `db.pack` in `game_data`.
///
/// Every row of every `units_custom_battle_permissions_tables` file with the vanilla table
/// version is copied with `general_unit` set; rows that already were generals are also kept as
/// non-general copies after the rows of their file, exactly as in the original manager. Sources
/// are the enabled mods in priority order (a pack name enabled twice is read once), then vanilla
/// `db.pack`. Mod files with another version or that cannot be read are skipped like the original
/// manager does; a missing or unreadable vanilla table is an error because it defines the version.
///
/// Unlike the original, the row count is the real number of rows (it divided the cell count by
/// the 10 fields of table version 10, overstating version 11 tables), the version is the vanilla
/// one rather than a hard-coded 11, and strings are copied as stored instead of being re-encoded
/// as ASCII. For ASCII data of version 11 the header and rows are otherwise byte-for-byte equal.
pub fn make_units_generals_table(
    game_data: &Path,
    catalog: &Catalog,
    order: &[usize],
    enabled: &HashSet<usize>,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let vanilla = game_data.join("db.pack");
    let (mut reader, entries) = pack::Reader::open_with_index(&vanilla)?;
    let first = table_entries(&entries).next().ok_or_else(|| {
        Error::Invalid(crate::message!(
            "{} has no {}",
            "в {} нет {}",
            vanilla.display(),
            TABLE
        ))
    })?;
    let version = read_header(&reader.read(first)?)?.version;
    let definition = schema()?
        .resolve(TABLE, version)
        .ok_or_else(|| unknown_version(TABLE, version))?;
    let general = definition.field(GENERAL_FIELD).ok_or_else(|| {
        Error::Invalid(crate::message!(
            "{} has no {} field",
            "в {} нет поля {}",
            TABLE,
            GENERAL_FIELD
        ))
    })?;
    let transform = Transform {
        version,
        definition,
        general,
    };

    let mut output = Output {
        rows: 0,
        data: Vec::new(),
    };
    let mut names = HashSet::new();
    let mods = order
        .iter()
        .filter(|index| enabled.contains(index))
        .filter_map(|&index| catalog.mods.get(index))
        .filter(|item| names.insert(item.name.to_lowercase()));
    for item in mods {
        // A broken mod must not block the launch; the original manager skips it as well.
        if let Ok((mut reader, entries)) = pack::Reader::open_with_index(&item.path) {
            for entry in table_entries(&entries) {
                if cancelled.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
                if let Ok(bytes) = reader.read(entry) {
                    // Unparseable files are skipped whole, so no partial rows reach the output.
                    let _ = transform.append(&bytes, &mut output);
                }
            }
        }
    }
    for entry in table_entries(&entries) {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        transform.append(&reader.read(entry)?, &mut output)?;
    }
    Ok(encode(definition.version, &output))
}

fn table_entries(entries: &[pack::PackedFile]) -> impl Iterator<Item = &pack::PackedFile> {
    entries.iter().filter(|entry| {
        !entry.name.ends_with(".rpfm_reserved")
            && super::table_name(&entry.name).as_deref() == Some(TABLE)
    })
}

struct Transform {
    version: Option<i32>,
    definition: &'static super::Definition,
    general: usize,
}

impl Transform {
    /// Appends one file's rows; returns `Ok(false)` for a file of another version.
    fn append(&self, bytes: &[u8], output: &mut Output) -> Result<bool> {
        let header = read_header(bytes)?;
        if header.version != self.version {
            return Ok(false);
        }
        let mut rows = Rows::new(bytes, &header, self.definition)?;
        let mut data = Vec::with_capacity(bytes.len());
        let mut generals = Vec::new();
        let mut count = 0u32;
        let mut spans = Vec::with_capacity(self.definition.fields.len());
        while rows.next_into(&mut spans)? {
            let (Some(first), Some(last), Some(flag)) =
                (spans.first(), spans.last(), spans.get(self.general))
            else {
                continue;
            };
            let row = rows.raw(first.start..last.end);
            let flag = flag.start - first.start;
            let was_general = row.get(flag) == Some(&1);
            let start = data.len();
            data.extend_from_slice(row);
            if let Some(byte) = data.get_mut(start + flag) {
                *byte = 1;
            }
            count += 1;
            if was_general {
                let start = generals.len();
                generals.extend_from_slice(row);
                if let Some(byte) = generals.get_mut(start + flag) {
                    *byte = 0;
                }
                count += 1;
            }
        }
        output.rows = output.rows.checked_add(count).ok_or_else(|| {
            Error::Invalid(crate::message!("too many rows", "слишком много строк"))
        })?;
        output.data.extend_from_slice(&data);
        output.data.extend_from_slice(&generals);
        Ok(true)
    }
}

/// The header layout of the original start pack writer: GUID, version, the `1` byte, row count.
fn encode(version: i32, output: &Output) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(91 + output.data.len());
    bytes.extend_from_slice(&GUID_MARKER);
    bytes.extend_from_slice(&(GUID.len() as u16).to_le_bytes());
    for unit in GUID.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes.extend_from_slice(&VERSION_MARKER);
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&output.rows.to_le_bytes());
    bytes.extend_from_slice(&output.data);
    bytes
}
