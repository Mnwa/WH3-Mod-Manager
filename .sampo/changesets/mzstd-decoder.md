---
cargo/wh3-core: patch (Changed)
---

Decode Zstandard pack entries and the embedded DB schema with `mzstd` instead of `ruzstd`. Entries decode in one pass into the output buffer, capped at the declared size, and must contain exactly one frame; bytes after it are now reported as corruption. Decoding the embedded schema takes 1.2 ms instead of 3.1 ms (median of 30 release-mode runs on an Apple M5 Pro).
