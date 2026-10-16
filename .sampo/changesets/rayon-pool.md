---
cargo/wh3-core: patch (Changed)
---

Conflict checks size their pack readers to the enclosing rayon pool when they run inside one (still capped at eight), so the desktop's two-thread pool while the game runs also limits them; outside a pool they use every core as before. Adds the `rayon` dependency.
