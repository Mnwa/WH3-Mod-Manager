---
cargo/wh3-mod-manager: patch (Fixed)
cargo/wh3-core: patch (Fixed)
---

Import the original manager's load order correctly: its presets are now ordered the way
it launches them (name order with pinned positions) instead of by list position. Hidden
and always-enabled mods and the game start options are imported as well.
