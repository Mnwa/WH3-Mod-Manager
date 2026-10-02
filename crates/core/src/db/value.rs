//! Decoded cell values.
use super::{FieldType, cursor::utf16};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    /// Packed RGB colour as stored.
    Colour(u32),
    Text(String),
    /// An optional field whose value is absent.
    Missing,
}

fn array<const N: usize>(raw: &[u8]) -> [u8; N] {
    raw.first_chunk::<N>().copied().unwrap_or([0; N])
}

impl Value {
    /// `raw` must be a span validated by the row cursor, prefixes included.
    pub(super) fn decode(kind: FieldType, raw: &[u8]) -> Self {
        let rest = raw.get(1..).unwrap_or_default();
        let present = raw.first() == Some(&1);
        match kind {
            FieldType::Boolean => Self::Bool(present),
            FieldType::I16 => Self::I16(i16::from_le_bytes(array(raw))),
            FieldType::I32 => Self::I32(i32::from_le_bytes(array(raw))),
            FieldType::I64 => Self::I64(i64::from_le_bytes(array(raw))),
            FieldType::F32 => Self::F32(f32::from_le_bytes(array(raw))),
            FieldType::F64 => Self::F64(f64::from_le_bytes(array(raw))),
            FieldType::ColourRgb => Self::Colour(u32::from_le_bytes(array(raw))),
            FieldType::StringU8 => {
                Self::Text(String::from_utf8_lossy(raw.get(2..).unwrap_or_default()).into_owned())
            }
            FieldType::StringU16 => Self::Text(utf16(raw.get(2..).unwrap_or_default())),
            FieldType::OptionalStringU8 if present => Self::decode(FieldType::StringU8, rest),
            FieldType::OptionalStringU16 if present => Self::decode(FieldType::StringU16, rest),
            FieldType::OptionalI16 if present => Self::decode(FieldType::I16, rest),
            FieldType::OptionalI32 if present => Self::decode(FieldType::I32, rest),
            FieldType::OptionalI64 if present => Self::decode(FieldType::I64, rest),
            _ => Self::Missing,
        }
    }
}

/// Key text: floats use three decimals like the original manager's `resolveKeyValue`
/// (`packFileSerializer.ts:723`), and an absent optional value is empty.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::I16(value) => write!(f, "{value}"),
            Self::I32(value) => write!(f, "{value}"),
            Self::I64(value) => write!(f, "{value}"),
            Self::F32(value) => write!(f, "{value:.3}"),
            Self::F64(value) => write!(f, "{value:.3}"),
            Self::Colour(value) => write!(f, "{value:06X}"),
            Self::Text(value) => f.write_str(value),
            Self::Missing => Ok(()),
        }
    }
}
