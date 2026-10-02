//! Load order rules: "pack A loads before pack B", written by the user or shipped by a pack in
//! `whmm\load_order.whmm`.
//!
//! Ported from the original manager's `loadOrderRules.ts` and `loadOrderRulesFile.ts`, but applied
//! to this application's explicit load order (a list of catalog indices) instead of a name sort
//! with pinned positions spliced in afterwards.
//!
//! "A before B" is purely positional: A is placed at a lower index than B, i.e. earlier in the list
//! and earlier in `used_mods.txt`. The original manager contradicts itself about what that means for
//! priority: `loadOrderRules.ts:5-7` and `nodeGraph/packPriority.ts:4` say the later pack wins,
//! while the rule-tab tooltips and the rules-file header (`LoadOrderRulesTab.tsx:365,372`,
//! `loadOrderRulesFile.ts:116`) say the earlier pack overrides. Its code only ever compares
//! positions, so this port keeps the positional meaning. In this application a lower position is a
//! higher priority (see `conflict`), so here "A before B" means A overrides B, matching the
//! original tooltips and the file header that mod authors read.
mod apply;
mod constraints;
mod graph;
mod packs;
mod parse;
mod resolve;
mod rule;

pub use apply::{Applied, apply};
pub use packs::pack_rules;
pub use parse::{RULES_FILE_PATH, is_rules_file_path, parse_rules_bytes, parse_rules_file};
pub use resolve::{DropReason, Resolution, resolve};
pub use rule::{Rule, RuleKey, RuleSource, normalize_pack_name, pack_key};
