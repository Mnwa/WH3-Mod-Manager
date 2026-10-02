---
cargo/wh3-mod-manager: minor (Added)
cargo/wh3-core: minor (Added)
---

Add Steam Workshop integration: titles, authors, tags, requirements and update times are
refreshed from Steam; mods can be updated, re-downloaded, subscribed and unsubscribed
(with confirmation) from the list; missing requirements can be installed and are enabled
once downloaded; Steam collections can be imported as presets. Steam calls run in a
short-lived worker process so the manager never appears to Steam as the running game.
Release archives now include `steam_api64.dll`.
