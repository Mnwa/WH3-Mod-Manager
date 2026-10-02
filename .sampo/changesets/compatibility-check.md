---
cargo/wh3-mod-manager: minor (Added)
cargo/wh3-core: minor (Added)
---

Replace the overlap list with a compatibility check that reports overwritten files with
the winning mod, colliding DB row keys with the
winning row (parsed with the bundled WH3 schema, including compressed packs), missing pack dependencies and Steam
requirements, and conflicting start positions, with per-row badges. Workshop mods older than
the last game update that overwrite vanilla DB or Lua files are marked as possibly outdated.
