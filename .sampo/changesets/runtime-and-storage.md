---
cargo/wh3-mod-manager: minor (Changed)
cargo/wh3-core: minor (Changed)
---

Update GPUI Kit to 0.7 and use mimalloc as the GUI's allocator. The library file moves
to the WHM3 format to store hidden and always-enabled mods, start options and load-order
rules; WHM1 libraries are migrated on the next save and the previous file is kept as the backup.
`WH3MM_HOME` selects a different data folder, for example for a portable copy.
