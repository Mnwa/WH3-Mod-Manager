//! Row iteration over a table's binary data, shared by the table reader, the key collision check
//! and the units-to-generals writer.
use super::{Definition, FieldType, Header, Value, cursor::Cursor};
use crate::{Error, Result};
use std::ops::Range;

pub(crate) struct Rows<'a> {
    cursor: Cursor<'a>,
    bytes: &'a [u8],
    definition: &'a Definition,
    remaining: u32,
}

impl<'a> Rows<'a> {
    /// Rejects row counts the data cannot hold before anything is allocated for them.
    pub(crate) fn new(
        bytes: &'a [u8],
        header: &Header,
        definition: &'a Definition,
    ) -> Result<Self> {
        let cursor = Cursor::at(bytes, header.data_start);
        let min_row: usize = definition
            .fields
            .iter()
            .map(|field| field.kind.min_size())
            .sum();
        let rows = header.rows as usize;
        let fits = match min_row {
            0 => rows == 0,
            size => rows <= cursor.remaining() / size,
        };
        if !fits {
            return Err(Error::Invalid(crate::message!(
                "DB table declares {} rows but has only {} bytes of data",
                "в таблице DB объявлено строк: {}, но данных только {} байт",
                header.rows,
                cursor.remaining()
            )));
        }
        Ok(Self {
            cursor,
            bytes,
            definition,
            remaining: header.rows,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.remaining as usize
    }

    /// Reads the next row into `spans`, one raw byte range per field; `false` after the last row.
    ///
    /// After the last row the data must be fully consumed: leftover bytes mean the definition
    /// does not describe the file, and every value read with it would be suspect.
    pub(crate) fn next_into(&mut self, spans: &mut Vec<Range<usize>>) -> Result<bool> {
        spans.clear();
        if self.remaining == 0 {
            if self.cursor.remaining() != 0 {
                return Err(Error::Invalid(crate::message!(
                    "DB table has {} unread bytes after the last row",
                    "в таблице DB после последней строки осталось байт: {}",
                    self.cursor.remaining()
                )));
            }
            return Ok(false);
        }
        for field in &self.definition.fields {
            spans.push(self.cursor.field(field.kind)?);
        }
        self.remaining -= 1;
        Ok(true)
    }

    pub(crate) fn raw(&self, span: Range<usize>) -> &'a [u8] {
        // Spans come from this cursor and are therefore always in bounds.
        self.bytes.get(span).unwrap_or_default()
    }

    pub(crate) fn value(&self, index: usize, span: Range<usize>) -> Value {
        let kind = self
            .definition
            .fields
            .get(index)
            .map_or(FieldType::Unsupported, |field| field.kind);
        Value::decode(kind, self.raw(span))
    }
}
