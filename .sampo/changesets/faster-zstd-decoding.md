---
cargo/wh3-mod-manager: patch (Changed)
---

Compatibility checks decode Zstandard-compressed mod files with a faster decoder. A compressed file with extra bytes after its data is now reported as corrupt instead of being read partially.
