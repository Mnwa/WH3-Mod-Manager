---
cargo/wh3-mod-manager: minor (Added)
cargo/wh3-core: minor (Added)
---

Add load-order rules like the original manager: "load before/after" rules from the row
menu, rules shipped by mods in `whmm\load_order.whmm` (each can be switched off), and
pinned positions for mods moved by hand. Rules are applied after changes and before
every launch, and the original's rules are imported from its configuration.
