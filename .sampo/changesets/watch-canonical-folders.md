---
cargo/wh3-core: patch (Fixed)
---

Keep automatic mod and save updates working when watched folders use symbolic links,
including macOS paths under `/var`. Folders whose real path cannot be resolved, such as
some RAM, network or virtual drives on Windows, are still watched by their configured path.
