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
| Enable/disable | Checkboxes, multi-selection (Ctrl/Shift+click, Ctrl+A, Space), "Enable/Disable all shown" for the current search results, all/enabled/disabled/hidden filters with counts, category filter |
| Load order | Drag and drop (also of a selection) in load-order view, Top/Up/Down/Bottom, Alt+arrows, Alt+Home/End and context menu; hand-placed mods are pinned like the original's `loadOrder` |
| Load-order rules | User "load before/after" rules and pack rules from `whmm\load_order.whmm`; contradictions and cycles dropped as in the original; individual pack rules can be switched off; applied after changes and before launch without moving pinned mods; imported from the original config |
| Sorting | Order, enabled, title, pack, author, updated, size; ascending/descending; never changes the launch order; a sorted view says so and offers a way back to the load order |
| Context menu | Enable/disable, move to top/bottom, keep always enabled, hide, categories, Workshop page, open in Steam, show in folder, copy path/pack name, pack files |
| Always enabled / hidden | Stored per pack name; presets and bulk actions keep always-enabled mods on; hiding disables unless always enabled |
| Categories | Edit per mod (dialog or quick toggles), sidebar filter with counts; stored as user metadata |
| Presets | Create/update by name, apply, enable its mods (merge), disable its mods (subtract), replace, delete, JSON import/export, automatic "On Last Game Launch"; the original's version-2 presets are ordered with its `sortByNameAndLoadOrder` (verified against a real 228-mod config) |
| Legacy presets | Full Mod records and compact entries; old sparse loadOrder reconstructed from names and pinned positions |
| Missing mods | Report on preset application; missing Steam requirements can be subscribed and are enabled after download |
| Original metadata | Source-only `wh3-meta` CLI (not in release assets), GUI export from config, import of v3 games.wh3 and legacy configurations, including rules, disabled rules, hidden and always-enabled mods and start options |
| Titles, authors, categories, tags | Imported, stored in binary format and searchable |
| Persistence | rkyv WHM4 (hidden, always-enabled, start options including Workshop staging and priority, load-order rules) with WHM1–WHM3 migration, bytecheck, CRC32, atomic replacement and previous-generation backup; `WH3MM_HOME` overrides the data folder |
| Languages | EN/RU switch in the top bar; the system display language is used until the user picks one (stored in the WHP2 preferences record); translated labels, reports, errors and edit menus |
| List layout | View menu: one list or two lists (not enabled \| enabled in load order; ticking or dropping a mod moves it across, drag reorders the right pane), category groups (each mod under every category, uncategorized first, collapsed at first, per-group enable/disable), row size Compact/Comfortable/Roomy, resizable Pack/Author/Updated/Size columns (drag the edge, double-click resets); all stored in WHP2 |
| Steam libraries | Standard paths and libraryfolders.vdf discovery; manual folder selection |
| Scanning | Data, Workshop and custom folders; cycle/path deduplication; per-file error report |
| Pack files | PFH4/PFH5 headers/indexes, including hashed-name PFH5; payload reading with zstd, LZ4 and LZMA decompression; minimal PFH5 writer for the start pack |
| DB tables | Embedded WH3 schema (RPFM-derived), strict DB header/row parsing of all field types used by WH3 |
| Pack inspection | Virtualized file names, sizes and compression flags |
| Compatibility | Overwritten files with the winning mod (lower order wins), shared DB tables and colliding DB row keys (including composite keys) with the winning row, missing pack dependencies and Steam requirements, startpos conflicts; per-row badges and report tabs |
| Outdated mods | Workshop mods older than the last game update (`appmanifest_1142710.acf`) that overwrite vanilla DB or Lua files, as the original; Workshop revisions newer than the installed copy |
| Game start options | Skip intro movies, script logging, auto-start custom battle and make units generals written to `!!!!out.pack` in the manager's own `temp_packs` folder and loaded last, as in the original; raise the game's process priority to High once it starts (reported once if Windows refuses); close manager on Play; all under Settings |
| Workshop staging | Settings → Workshop mods at launch: load from the Workshop folder, copy, or link enabled Workshop mods into `<game>\whmm_copied_mods` (the original's folder); mods also in `data` stay there; unchanged copies (size and time) and correct links are reused, unselected packs pruned; progress in the status bar, cancellable; optional deletion after the game exits (also for a folder left by an earlier session) and a "Delete prepared mods now" action with the folder size |
| Data folder | Folders → Game data folder and the row menu: copy or link mods into `data` (never overwriting), remove copies that also exist elsewhere or only links (with confirmation), delete a data pack; link availability (administrator or Developer Mode) is probed once; data packs come first by name, data links are shown in blue and shadowed Workshop copies get a badge |
| Saves | Continue (newest save); its drop-down loads any recent save or enables the mods recorded in it |
| Workshop | Steamworks through a short-lived worker process: titles, authors, tags, requirements, update times, update/re-download, subscribe, unsubscribe (with confirmation), missing requirements, Steam collection import; tested against a live Steam account |
| WH3 launch | Windows, direct `Warhammer3.exe` launch with a separate `wh3_rust_mods.txt`; real-game verification remains outstanding |
| Guidance | First-start screen (game folder or Workshop link, three-step checklist, import from the original); empty search and filter states with a next action; a bar under the list that explains the table when nothing is selected and shows the selection's actions otherwise; one-line explanations on report tabs |
| Play | Saves the library before launching, so the list survives closing the manager with the game |
| Windows executable | Application icon (rendered from `crates/desktop/assets/logo.svg`) and version information (product, version from the release tag, copyright) embedded at build time |
| Live updates | `.pack` changes in every mod folder (and Workshop item folders appearing or disappearing) trigger a rescan after two quiet seconds, keeping unsaved state and the selection; an enabled mod whose file disappears and comes back (a Workshop update) is enabled again next to its previous neighbour; Play/Continue wait until changed files are rescanned, like the original's launch delay; new `*.save` files refresh the save list; watched roots are canonicalized so events match folders reached through symbolic links, including macOS `/var` paths, and folders whose real path cannot be resolved (some Windows RAM, network or virtual drives) are watched by their configured path |
| Game process | `tasklist` every two seconds, as the original; while the game runs Play/Continue queue a relaunch for when it closes (click again to cancel), a close button ends it (`taskkill /F /T`, with confirmation) and Play never starts a second copy |
| Shared mod lists | The original's text format (`id;order|local:<name>:<id>;order`); export writes every position so either manager reproduces the exact order; import matches by name or Workshop id, subscribes missing Workshop mods and applies the list after download, keeping the previous list as the preset "Before shared list" |
| Problem-mod search | Guided bisect instead of the original's pair of presets: requirement-linked mods stay together, each step enables half of the suspects, the player answers "still there"/"gone" (with undo); the list is kept as the preset "Before problem search" and restored with or without the found mods |
| Workshop repair | "Reinstall from Workshop" unsubscribes, waits until Steam removed the files, subscribes again and re-enables the mod, like the original's force-resubscribe |
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
- Workshop staging does not compress staged copies and has no up-front free-space
  check; a full disk is reported by the failing copy. The original's manifest is not
  written; copies are reused by size and modification time.
- Two-list panes have no separate sort settings (the left pane follows the table sort,
  the right pane is always the load order) and no keyboard placement mode; reorder by
  dragging. Group collapse state is not persisted, as in the original.
- The original's per-folder source priority, "Copy Mods to New Folder" and merged-pack
  handling are not ported.
- The problem-mod search assumes one mod (or one requirement-linked group) causes the
  problem; problems that need two unrelated mods together are not found.
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
2. Add missing-reference and script checks to the compatibility report.
3. Port the original pack/DB/LOC editors and game tools.
