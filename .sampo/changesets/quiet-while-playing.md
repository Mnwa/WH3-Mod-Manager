---
cargo/wh3-mod-manager: patch (Changed)
---

While the game is running, the manager does its background work (Workshop data, mod scans, compatibility checks) on two threads so it does not take processor time from the game, and returns to full speed once the game closes.
