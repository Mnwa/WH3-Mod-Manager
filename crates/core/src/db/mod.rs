//! DB table files: the embedded schema, a bounded reader and the units-to-generals table.
//!
//! Every input is untrusted mod data, so all lengths and row counts are validated against the
//! remaining bytes before use, and a file whose rows do not consume it exactly is rejected.
mod cursor;
mod generals;
mod header;
mod rows;
mod schema;
mod value;

pub use generals::{UNITS_GENERALS_PATH, make_units_generals_table};
pub use header::{Header, read_header};
pub use schema::{Definition, Field, FieldType, Schema, schema};
pub use value::Value;

use crate::{Error, Result};

/// A table file decoded with the schema definition for its version.
#[derive(Debug)]
pub struct Table {
    /// Table folder name, e.g. `main_units_tables`.
    pub name: String,
    pub guid: Option<String>,
    /// The version of the definition used; 0 for files without a version marker.
    pub version: i32,
    pub definition: &'static Definition,
    /// Cells in the order of `definition.fields`.
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    /// Key values of one row in key field order; `None` for a row index out of range.
    pub fn key(&self, row: usize) -> Option<Vec<String>> {
        let row = self.rows.get(row)?;
        Some(
            self.definition
                .key_fields()
                .filter_map(|index| row.get(index).map(ToString::to_string))
                .collect(),
        )
    }
}

/// The table folder of a packed DB path, e.g. `units_tables` for `db\units_tables\my_mod`.
///
/// Only files under `db\<table>\` are loaded by the game; like the original manager's
/// `parseLiveDBTablePath`, a nested file name keeps the table folder. Case and separators are
/// normalized.
pub fn table_name(path: &str) -> Option<String> {
    let path = path.replace('/', "\\").to_lowercase();
    let rest = path.strip_prefix("db\\")?;
    let (table, file) = rest.split_once('\\')?;
    (!table.is_empty() && !file.is_empty()).then(|| table.to_owned())
}

pub(crate) fn unknown_version(table: &str, version: Option<i32>) -> Error {
    Error::Invalid(crate::message!(
        "{} version {} is not in the bundled schema",
        "{} версии {} нет во встроенной схеме",
        table,
        version.unwrap_or(0)
    ))
}

/// Decodes a DB table file of `table` (the folder name, e.g. `main_units_tables`).
pub fn read_table(bytes: &[u8], table: &str) -> Result<Table> {
    let header = read_header(bytes)?;
    let definition = schema()?
        .resolve(table, header.version)
        .ok_or_else(|| unknown_version(table, header.version))?;
    let mut rows = rows::Rows::new(bytes, &header, definition)?;
    let mut decoded = Vec::with_capacity(rows.len());
    let mut spans = Vec::with_capacity(definition.fields.len());
    while rows.next_into(&mut spans)? {
        decoded.push(
            spans
                .drain(..)
                .enumerate()
                .map(|(index, span)| rows.value(index, span))
                .collect(),
        );
    }
    Ok(Table {
        name: table.to_owned(),
        guid: header.guid,
        version: definition.version,
        definition,
        rows: decoded,
    })
}

/// Key of every row, in file order. Only key cells are decoded.
///
/// A definition without key fields yields an empty key per row; callers skip such tables.
pub(crate) fn read_keys(bytes: &[u8], table: &str) -> Result<Vec<Vec<String>>> {
    let header = read_header(bytes)?;
    let definition = schema()?
        .resolve(table, header.version)
        .ok_or_else(|| unknown_version(table, header.version))?;
    let key_fields: Vec<usize> = definition.key_fields().collect();
    let mut rows = rows::Rows::new(bytes, &header, definition)?;
    let mut keys = Vec::with_capacity(rows.len());
    let mut spans = Vec::with_capacity(definition.fields.len());
    while rows.next_into(&mut spans)? {
        keys.push(
            key_fields
                .iter()
                .filter_map(|&index| {
                    let span = spans.get(index)?.clone();
                    Some(rows.value(index, span).to_string())
                })
                .collect(),
        );
    }
    Ok(keys)
}
