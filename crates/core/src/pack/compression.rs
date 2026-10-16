//! Compressed PFH5 entries.
//!
//! A compressed entry starts with its uncompressed size (`u32`, little-endian), followed by one
//! stream. RPFM and the game write Zstandard or LZ4 frames, recognised by their frame magic, and
//! older packs use CA's LZMA1 layout: the 5 property bytes immediately followed by the raw
//! stream, without the 8-byte size of the `.lzma` container. The original manager probes for the
//! same frame magics after the size (`decompressPackedPayload`, `packFileSerializer.ts:107`).
use crate::{Error, Result};
use std::io::{self, Read, Write};

/// Largest payload, stored or decompressed, that is read into memory. DB tables and scripts are
/// far smaller; the limit only stops hostile size fields from exhausting memory.
pub const MAX_ENTRY_SIZE: u64 = 256 * 1024 * 1024;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const LZ4_MAGIC: [u8; 4] = [0x04, 0x22, 0x4d, 0x18];
const LZMA_PROPERTIES: usize = 5;
/// Capacity reserved up front; larger outputs grow only as real data arrives.
const INITIAL_CAPACITY: usize = 4 * 1024 * 1024;

fn corrupt(detail: impl std::fmt::Display) -> Error {
    Error::Pack(crate::message!(
        "compressed file is corrupt: {}",
        "сжатый файл повреждён: {}",
        detail
    ))
}

/// Decodes a stored entry whose index flag says it is compressed.
pub fn decompress(stored: &[u8]) -> Result<Vec<u8>> {
    let (size, stream) = stored.split_first_chunk::<4>().ok_or_else(|| {
        Error::Pack(crate::message!(
            "compressed file has no size",
            "у сжатого файла нет размера"
        ))
    })?;
    let size = u32::from_le_bytes(*size);
    if u64::from(size) > MAX_ENTRY_SIZE {
        return Err(Error::Pack(crate::message!(
            "compressed file is too large: {} bytes",
            "сжатый файл слишком большой: {} байт",
            size
        )));
    }
    let size = size as usize;
    let output = match stream.first_chunk::<4>() {
        Some(&ZSTD_MAGIC) => zstd(stream, size)?,
        Some(&LZ4_MAGIC) => read_bounded(lz4_flex::frame::FrameDecoder::new(stream), size)?,
        _ => lzma(stream, size)?,
    };
    if output.len() != size {
        return Err(Error::Pack(crate::message!(
            "compressed file is corrupt: expected {} bytes, got {}",
            "сжатый файл повреждён: ожидалось {} байт, получено {}",
            size,
            output.len()
        )));
    }
    Ok(output)
}

/// Decodes the one frame straight into its output, which also serves as the frame's history:
/// memory grows with the decoded data, which the output limit caps, not with the window a frame
/// declares. Vanilla packs declare 128 MiB windows for small tables, so the window limit is the
/// format's maximum.
fn zstd(stream: &[u8], size: usize) -> Result<Vec<u8>> {
    const CONFIG: mzstd::DecoderConfig = match mzstd::DecodeLimits::DEFAULT
        .with_max_window_log(mzstd::DecodeLimits::MAX_WINDOW_LOG)
    {
        Ok(limits) => mzstd::DecoderConfig::DEFAULT
            .with_frame_mode(mzstd::FrameMode::Single)
            .with_limits(limits),
        Err(_) => panic!("the maximum window log is valid"),
    };
    let limits = CONFIG.limits().with_max_output_bytes(Some(size as u64));
    mzstd::Decompressor::new(CONFIG.with_limits(limits))
        .decompress(stream)
        .map_err(corrupt)
}

/// Reads at most one byte more than expected, so an oversized stream is detected without
/// decoding it completely.
fn read_bounded(reader: impl Read, size: usize) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(size.min(INITIAL_CAPACITY));
    reader
        .take(size as u64 + 1)
        .read_to_end(&mut output)
        .map_err(corrupt)?;
    Ok(output)
}

fn lzma(stream: &[u8], size: usize) -> Result<Vec<u8>> {
    if stream.len() < LZMA_PROPERTIES {
        return Err(corrupt("LZMA header is truncated"));
    }
    let options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(size as u64)),
        // The dictionary never needs to exceed the output, whatever the header claims.
        memlimit: Some(size.max(4096)),
        allow_incomplete: false,
    };
    let mut output = Bounded {
        bytes: Vec::with_capacity(size.min(INITIAL_CAPACITY)),
        limit: size,
    };
    lzma_rs::lzma_decompress_with_options(&mut io::Cursor::new(stream), &mut output, &options)
        .map_err(corrupt)?;
    Ok(output.bytes)
}

/// Output sink that refuses to grow past the declared size.
struct Bounded {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for Bounded {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.bytes.len() + data.len() > self.limit {
            return Err(io::Error::other("output exceeds the declared size"));
        }
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
