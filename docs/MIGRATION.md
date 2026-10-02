# Migration status

The agreed first stage covers the mod manager; the original application's other
tools are later work. Reference: Shazbot/WH3-Mod-Manager, commit
`083c7e2a570991227eda0d071611e85d73d345f8`.
This is a standalone Rust application; Electron is not included in the distribution.

| Capability | Rust implementation |
|---|---|
| Mod list | Native GPUI with virtualized, fixed-height rows and an always-visible scrollbar; no WebView |
| Columns | Load order, enabled, thumbnail, title with badges, pack (coloured by source), author, relative update time, size |
| Thumbnails | Original rules: same-stem `.png`/`.jpg` next to the pack, first image in a Workshop item folder, Data mods borrow the Workshop copy's image; decoded lazily by GPUI |
| Large libraries | Synthetic 1k/10k/100k tests; background scanning, search and sorting |
| Search | Unicode case-insensitive, multiple terms; title, file, ID, author, categories and tags; `/regex/` (case-insensitive, invalid patterns fall back to substring) |
| Enable/disable | Checkboxes, multi-selection (Ctrl/Shift+click, Ctrl+A, Space), bulk actions on search results, all/enabled/disabled/hidden filters, category filter |
| Load order | Drag and drop (also of a selection) in load-order view, Top/Up/Down/Bottom, Alt+arrows, Alt+Home/End and context menu; hand-placed mods are pinned like the original's `loadOrder` |
| Load-order rules | User "load before/after" rules and pack rules from `whmm\load_order.whmm`; contradictions and cycles dropped as in the original; individual pack rules can be switched off; applied after changes and before launch without moving pinned mods; imported from the original config |
| Sorting | Order, enabled, title, pack, author, updated, size; ascending/descending; never changes the launch order |
| Context menu | Enable/disable, move to top/bottom, keep always enabled, hide, categories, Workshop page, open in Steam, show in folder, copy path/pack name, pack files |
| Always enabled / hidden | Stored per pack name; presets and bulk actions keep always-enabled mods on; hiding disables unless always enabled |
| Categories | Edit per mod (dialog or quick toggles), sidebar filter with counts; stored as user metadata |
| Presets | Create/update by name, apply, enable its mods (merge), disable its mods (subtract), replace, delete, JSON import/export, automatic "On Last Game Launch"; the original's version-2 presets are ordered with its `sortByNameAndLoadOrder` (verified against a real 228-mod config) |
| Legacy presets | Full Mod records and compact entries; old sparse loadOrder reconstructed from names and pinned positions |
| Missing mods | Report on preset application; missing Steam requirements can be subscribed and are enabled after download |
| Original metadata | Source-only `wh3-meta` CLI (not in release assets), GUI export from config, import of v3 games.wh3 and legacy configurations, including rules, disabled rules, hidden and always-enabled mods and start options |
| Titles, authors, categories, tags | Imported, stored in binary format and searchable |
| Persistence | rkyv WHM3 (hidden, always-enabled, start options, load-order rules) with WHM1/WHM2 migration, bytecheck, CRC32, atomic replacement and previous-generation backup; `WH3MM_HOME` overrides the data folder |
| Languages | English/Russian switch, persisted binary preference, translated labels, reports, errors and edit menus |
| Steam libraries | Standard paths and libraryfolders.vdf discovery; manual folder selection |
| Scanning | Data, Workshop and custom folders; cycle/path deduplication; per-file error report |
| Pack files | PFH4/PFH5 headers/indexes, including hashed-name PFH5; payload reading with zstd, LZ4 and LZMA decompression; minimal PFH5 writer for the start pack |
| DB tables | Embedded WH3 schema (RPFM-derived), strict DB header/row parsing of all field types used by WH3 |
| Pack inspection | Virtualized file names, sizes and compression flags |
| Compatibility | Overwritten files with the winning mod (lower order wins), shared DB tables and colliding DB row keys (including composite keys) with the winning row, missing pack dependencies and Steam requirements, startpos conflicts; per-row badges and report tabs |
| Outdated mods | Workshop mods older than the last game update (`appmanifest_1142710.acf`) that overwrite vanilla DB or Lua files, as the original; Workshop revisions newer than the installed copy |
| Game start options | Skip intro movies, script logging, auto-start custom battle and make units generals written to `!!!!out.pack` in the manager's own `temp_packs` folder and loaded last, as in the original; close manager on Play |
| Saves | Continue (newest save), load any recent save, enable the mods recorded in a save |
| Workshop | Steamworks through a short-lived worker process: titles, authors, tags, requirements, update times, update/re-download, subscribe, unsubscribe (with confirmation), missing requirements, Steam collection import; tested against a live Steam account |
| WH3 launch | Windows, direct `Warhammer3.exe` launch with a separate `wh3_rust_mods.txt`; real-game verification remains outstanding |
| Self-update | GitHub Releases check, SHA-256 verification against `SHA256SUMS.txt`, replacement of the executable and `steam_api64.dll`, restart (Windows release builds) |
| Release preparation | Sampo changesets, synchronized Cargo versions and per-package changelogs in a draft release PR after successful main CI |
| Builds | `ci.yml` runs Ubuntu-only workspace Clippy and tests with `--all-targets`, plus doc tests, on main pushes/PRs; `release.yml` builds Windows x64 assets from an existing published GitHub Release and attaches exe, ZIP and checksums; unsigned |

## Limitations

- CI does not check Windows or macOS targets. Metal screenshot tests are skipped on
  Ubuntu and must be run locally on macOS; `--all-targets` selects Cargo targets,
  not operating systems.
- WH3/Steam is supported. Other Total War games and game launch through Linux/Proton
  or macOS are not ported.
- No DB/LOC editor, pack editor, mod merging, missing-reference/unique-index/script
  listener checks, game viewers (units, buildings, skills, maps), node flows, mod
  customization overwrites or game-entity customization.
- No dual-pane or category-grouped layout, resizable columns, row density settings,
  workshop staging, symbolic links/copy to data, bisecting, shared-mod-list strings,
  game process watching or force-resubscribe repair. No automatic folder watching; use
  Refresh.
- Pins keep their order relative to other pinned and unmoved mods instead of an
  absolute index; outdated detection has no `tw_updates` feed of patch-specific files.
- DB tables whose version is newer than the bundled schema are reported as unchecked.
- Packs with identical names are displayed separately, but original presets bind by
  name. The first discovered copy is selected; launching two enabled packs with the
  same name is rejected. Review the chosen paths after migration.
- Legacy presets without a version use case-insensitive name sorting. Non-ASCII
  ordering can differ from the original `Intl.Collator("en")` behavior.
- The game automatically loads movie packs in `data`. If one is disabled in the UI,
  launch is blocked with an explanation. The manager does not remove or move it.
- Indexes are limited to 128 MiB; PFH3/PFH6 are unsupported. Unsupported or damaged
  packs appear in the report. Viewing compressed file contents is not implemented.
- Metadata comes from saved configuration, not Electron's in-memory state. Original
  v3 usually saves title/author/categories/requirements; Workshop IDs and tags are
  copied only when present in a legacy full record. IDs can also come from Workshop paths.
- Writing the start pack is not retried; if it fails the launch is aborted with an error.
- Native file picker controls and OS diagnostic details follow the system locale.
  Imported names and third-party warnings are preserved verbatim.

## Next stages

1. Verify a real WH3 launch with start options and saves on Windows.
2. Add the dual-pane/category layouts and the remaining list interactions.
3. Add missing-reference and script checks to the compatibility report.
4. Port the original pack/DB/LOC editors and game tools.
