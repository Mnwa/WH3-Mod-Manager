# Binary persistence

The library is stored in the user's configuration directory:

- Windows: `%APPDATA%/wh3-mod-manager-rust/library.whmm`.
- macOS: `~/Library/Application Support/wh3-mod-manager-rust/library.whmm`.
- Linux: `$XDG_CONFIG_HOME/wh3-mod-manager-rust/library.whmm` (usually under `~/.config`).

JSON is used only for import/export. Mod files and pack payloads are not copied into the library.

## WHM1 library format

| Offset | Value |
|---|---|
| 0..4 | ASCII `WHM1`: signature and version |
| 4..8 | Payload length, little-endian u32 |
| 8..12 | Payload CRC32, little-endian u32 |
| 12.. | rkyv `StateRecord` |

The rkyv features fix the **data format**, not the Rust compiler version:
`little_endian`, `pointer_width_32`, `unaligned`, `bytecheck`. As in `cr-chat-desktop`,
the record schema lives separately from domain models, in `storage_format.rs`.
It contains paths, sources, current order/enabled states, presets and mod metadata.
Serialization borrows strings and writes directly after the header in the output
buffer. Reads validate length, checksum and rkyv bytecheck before reconstructing the
model. Neither `access_unchecked` nor application `unsafe` code is used.

Files are limited to 128 MiB. Corruption, unknown versions and unknown source codes
return an error. CRC32 detects accidental corruption; it is not a cryptographic
signature. The store is unencrypted: it holds game metadata rather than the medical
data or tokens handled by the reference project.

## Writing and recovery

Writes happen in the background: a temporary file in the same directory is flushed
with `sync_all`, then atomically replaces the destination. Before replacement, the
existing library is validated and copied to `library.whmm.bak`. An invalid existing
library blocks replacement, so an empty state after a load error cannot destroy it.

If the main library is damaged, close the application, keep a separate copy of the
damaged file, then restore `library.whmm.bak` as `library.whmm`. Recovery is deliberately
explicit so the loss of the latest saved generation is not hidden.

Schema changes require a new signature/version and an explicit migration from the
old schema. Domain models can change without implicitly changing the disk layout.
Tests cover Unicode/metadata round trips, every-byte corruption, truncation, refusal
to overwrite an invalid library, previous-generation backup and byte-for-byte
agreement with the frozen WHM1 fixture.

## WHP1 interface preferences

`preferences.whmp` lives beside the library. It is exactly five bytes: ASCII `WHP1`
followed by `0` for English or `1` for Russian. Missing preferences default to English;
unknown signatures, language codes, truncation and trailing bytes are rejected.

Language changes are saved atomically in the background, independently of library
saves. They do not alter the WHM1 schema or its backup. Demo mode does not read or
write persistent language preferences. Tests cover restart persistence, malformed
records and preservation of the neighboring library file.
