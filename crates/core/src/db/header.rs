//! The header in front of every DB table file.
//!
//! Layout, as read by the original manager (`readDBPackedFiles`, `packFileSerializer.ts:2877`)
//! and RPFM: an optional GUID (`FD FE FC FF`, `u16` length in UTF-16 units, UTF-16LE text), an
//! optional version (`FC FD FE FF`, `i32`), one byte that is always `1`, then the `u32` row count.
use super::cursor::Cursor;
use crate::{Error, Result};

pub(crate) const GUID_MARKER: [u8; 4] = [0xfd, 0xfe, 0xfc, 0xff];
pub(crate) const VERSION_MARKER: [u8; 4] = [0xfc, 0xfd, 0xfe, 0xff];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub guid: Option<String>,
    /// `None` when the file has no version marker; such files use the version 0 definition.
    pub version: Option<i32>,
    pub rows: u32,
    /// Offset of the first row.
    pub data_start: usize,
}

fn invalid_header(detail: crate::localization::Message) -> Error {
    Error::Invalid(crate::message!("DB header: ", "заголовок DB: ").concat(detail))
}

pub fn read_header(bytes: &[u8]) -> Result<Header> {
    let mut cursor = Cursor::new(bytes);
    let mut guid = None;
    let mut version = None;
    loop {
        match cursor.peek4() {
            Some(GUID_MARKER) if guid.is_none() => {
                cursor.take(4)?;
                let units = usize::from(cursor.u16()?);
                let text = cursor.take(units * 2)?;
                guid = Some(super::cursor::utf16(text));
            }
            Some(VERSION_MARKER) if version.is_none() => {
                cursor.take(4)?;
                version = Some(cursor.i32()?);
            }
            _ => break,
        }
    }
    match cursor.u8() {
        Ok(1) => {}
        Ok(other) => {
            return Err(invalid_header(crate::message!(
                "unexpected byte {:#04x} before the row count",
                "неожиданный байт {:#04x} перед числом строк",
                other
            )));
        }
        Err(e) => return Err(e),
    }
    let rows = cursor.u32()?;
    Ok(Header {
        guid,
        version,
        rows,
        data_start: cursor.position(),
    })
}
