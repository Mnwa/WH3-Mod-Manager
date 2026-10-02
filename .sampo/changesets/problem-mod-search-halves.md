---
cargo/wh3-mod-manager: patch (Fixed)
cargo/wh3-core: patch (Fixed)
---

Fix the problem-mod search naming half of a large mod list: mods linked through a shared framework or overhaul are now tested separately, each with the mods it requires. "Restore without it" also switches off the mods that require the culprit, and the search keeps track of mods when Workshop updates rescan the library mid-search.
