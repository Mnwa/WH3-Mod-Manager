//! Bounds-checked little-endian reads over untrusted table bytes.
use super::FieldType;
use crate::{Error, Result};
use std::ops::Range;

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

fn truncated(position: usize) -> Error {
    Error::Invalid(crate::message!(
        "DB table is truncated at byte {}",
        "таблица DB обрезана на байте {}",
        position
    ))
}

impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(super) fn at(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    pub(super) fn position(&self) -> usize {
        self.position
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    pub(super) fn peek4(&self) -> Option<[u8; 4]> {
        self.bytes.get(self.position..)?.first_chunk::<4>().copied()
    }

    pub(super) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(len)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| truncated(self.position))?;
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let position = self.position;
        self.take(N)?
            .first_chunk::<N>()
            .copied()
            .ok_or_else(|| truncated(position))
    }

    pub(super) fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    /// A `0`/`1` byte. Anything else means the schema does not match the data, so it is an error
    /// rather than a silently misaligned row.
    fn flag(&mut self) -> Result<bool> {
        let position = self.position;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(Error::Invalid(crate::message!(
                "DB table has an invalid boolean {} at byte {}",
                "в таблице DB неверное логическое значение {} на байте {}",
                other,
                position
            ))),
        }
    }

    /// Validates one field and returns its raw bytes, including length or presence prefixes.
    pub(super) fn field(&mut self, kind: FieldType) -> Result<Range<usize>> {
        let start = self.position;
        match kind {
            FieldType::Boolean => {
                self.flag()?;
            }
            FieldType::I16 => {
                self.take(2)?;
            }
            FieldType::I32 | FieldType::F32 | FieldType::ColourRgb => {
                self.take(4)?;
            }
            FieldType::I64 | FieldType::F64 => {
                self.take(8)?;
            }
            FieldType::StringU8 => self.string(1)?,
            FieldType::StringU16 => self.string(2)?,
            FieldType::OptionalStringU8 => self.optional(FieldType::StringU8)?,
            FieldType::OptionalStringU16 => self.optional(FieldType::StringU16)?,
            FieldType::OptionalI16 => self.optional(FieldType::I16)?,
            FieldType::OptionalI32 => self.optional(FieldType::I32)?,
            FieldType::OptionalI64 => self.optional(FieldType::I64)?,
            FieldType::Unsupported => {
                return Err(Error::Invalid(crate::message!(
                    "DB table uses a field type that is not supported",
                    "в таблице DB используется неподдерживаемый тип поля"
                )));
            }
        }
        Ok(start..self.position)
    }

    fn string(&mut self, unit: usize) -> Result<()> {
        let len = usize::from(self.u16()?);
        let text = self.take(len * unit)?;
        if unit == 1 && std::str::from_utf8(text).is_err() {
            return Err(Error::Invalid(crate::message!(
                "DB table has invalid UTF-8 text before byte {}",
                "в таблице DB некорректный текст UTF-8 перед байтом {}",
                self.position
            )));
        }
        Ok(())
    }

    fn optional(&mut self, kind: FieldType) -> Result<()> {
        if self.flag()? {
            self.field(kind)?;
        }
        Ok(())
    }
}

/// Lossy because GUIDs are informational and never written back from parsed text.
pub(super) fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .collect();
    String::from_utf16_lossy(&units)
}
