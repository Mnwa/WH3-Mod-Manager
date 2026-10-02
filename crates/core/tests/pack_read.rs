#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;

use std::fs;
use support::{Entry, lz4, lzma, pack_bytes, packed, plain, zstd};
use wh3_core::{Error, pack};

/// Compressible but not trivial, so every codec emits real matches.
fn sample(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| (i % 251) as u8 ^ (i / 1000) as u8)
        .collect()
}

#[test]
fn reads_plain_and_every_compressed_format() {
    let data = sample(300_000);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("mixed.pack");
    let entries = [
        plain("script/plain.lua", b"print('hi')"),
        packed("db\\zstd_tables\\data", zstd(&data)),
        packed("db\\lz4_tables\\data", lz4(&data)),
        packed("db\\lzma_tables\\data", lzma(&data)),
        packed("empty", zstd(&[])),
    ];
    fs::write(&path, pack_bytes(false, false, &entries)).unwrap();

    assert_eq!(
        pack::read_file(&path, "SCRIPT\\Plain.lua").unwrap(),
        b"print('hi')",
        "names match case-insensitively with either separator"
    );
    let names = [
        "db/zstd_tables/data",
        "db\\lz4_tables\\data",
        "db\\lzma_tables\\data",
        "missing",
        "empty",
    ];
    let files = pack::read_files(&path, &names).unwrap();
    assert_eq!(files[0].as_deref(), Some(&data[..]));
    assert_eq!(files[1].as_deref(), Some(&data[..]));
    assert_eq!(files[2].as_deref(), Some(&data[..]));
    assert_eq!(files[3], None);
    assert_eq!(files[4].as_deref(), Some(&[][..]));
    assert!(matches!(
        pack::read_file(&path, "missing"),
        Err(Error::Pack(_))
    ));
}

#[test]
fn reads_pfh4_and_timestamped_indexes() {
    let temp = tempfile::tempdir().unwrap();
    for (pfh4, timestamps) in [(true, false), (true, true), (false, true)] {
        let path = temp.path().join(format!("{pfh4}-{timestamps}.pack"));
        let entries = [plain("a.txt", b"first"), plain("b.txt", b"second")];
        fs::write(&path, pack_bytes(pfh4, timestamps, &entries)).unwrap();
        let files = pack::read_files(&path, &["b.txt", "a.txt"]).unwrap();
        assert_eq!(files[0].as_deref(), Some(&b"second"[..]));
        assert_eq!(files[1].as_deref(), Some(&b"first"[..]));
    }
}

#[test]
fn reader_reuses_one_handle_for_index_entries() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reuse.pack");
    let entries = [
        plain("a", b"1"),
        packed("b", lz4(b"22")),
        plain("c", b"333"),
    ];
    fs::write(&path, pack_bytes(false, false, &entries)).unwrap();
    let (mut reader, index) = pack::Reader::open_with_index(&path).unwrap();
    let contents: Vec<Vec<u8>> = index
        .iter()
        .rev()
        .map(|e| reader.read(e).unwrap())
        .collect();
    assert_eq!(contents, [b"333".to_vec(), b"22".to_vec(), b"1".to_vec()]);
}

fn rejects(stored: Vec<u8>) {
    assert!(
        matches!(pack::decompress(&stored), Err(Error::Pack(_))),
        "{} bytes starting {:02x?}",
        stored.len(),
        &stored[..stored.len().min(12)]
    );
}

#[test]
fn hostile_compressed_entries_are_rejected() {
    let data = sample(10_000);
    // Too short to hold the size, and a size above the limit.
    rejects(vec![1, 0]);
    rejects([u32::MAX.to_le_bytes().to_vec(), zstd(&data)[4..].to_vec()].concat());
    // The declared size disagrees with the stream. Raw LZMA has no length of its own, so like
    // the game it decodes exactly the declared size and only a longer claim is detectable.
    for (stream, framed) in [
        (zstd(&data), true),
        (lz4(&data), true),
        (lzma(&data), false),
    ] {
        let mut smaller = stream.clone();
        smaller[..4].copy_from_slice(&9_999u32.to_le_bytes());
        if framed {
            rejects(smaller);
        } else {
            assert_eq!(pack::decompress(&smaller).unwrap(), data[..9_999]);
        }
        let mut larger = stream.clone();
        larger[..4].copy_from_slice(&10_001u32.to_le_bytes());
        rejects(larger);
        // Streams cut inside the data fail, and flipped bytes fail cleanly instead of panicking.
        // (An LZ4 frame missing only its end mark still yields every declared byte.)
        for cut in [5, 9, stream.len() / 2] {
            rejects(stream[..cut].to_vec());
        }
        for position in (4..stream.len()).step_by(97) {
            let mut corrupt = stream.clone();
            corrupt[position] ^= 0x5a;
            let _ = pack::decompress(&corrupt);
        }
    }
    // Neither a zstd nor an LZ4 frame and not valid LZMA either.
    rejects([10u32.to_le_bytes().to_vec(), vec![0xff; 12]].concat());
}

#[test]
fn entries_beyond_a_replaced_pack_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("shrunk.pack");
    let entries = [Entry {
        name: "big",
        stored: vec![7; 4096],
        compressed: false,
    }];
    let bytes = pack_bytes(false, false, &entries);
    fs::write(&path, &bytes).unwrap();
    let (mut reader, index) = pack::Reader::open_with_index(&path).unwrap();
    // The pack is truncated after its index was read.
    fs::write(&path, &bytes[..bytes.len() - 100]).unwrap();
    assert!(matches!(reader.read(&index[0]), Err(Error::Pack(_))));
}
