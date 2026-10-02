---
cargo/wh3-core: minor (Added)
---

Add the native WH3 mod-management core: Steam/Data/Workshop scanning, Unicode search,
preset import/export and load order, PFH4/PFH5 index inspection, file-overlap reports,
and a separate Windows game launch script. Import saved original-manager metadata
and export it through the standalone `wh3-meta` CLI. Store libraries in the validated,
versioned WHM1 binary format with atomic saves and a previous-generation backup.
