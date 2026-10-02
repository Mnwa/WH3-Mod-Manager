# Migration status

The agreed first stage covers the mod manager; the original application's other
tools are later work. Reference: Shazbot/WH3-Mod-Manager, commit
`083c7e2a570991227eda0d071611e85d73d345f8`.
This is a standalone Rust application; Electron is not included in the distribution.

| Capability | Rust implementation |
|---|---|
| Mod list | Native GPUI with virtualized, fixed-height rows; no WebView |
| Large libraries | Synthetic 1k/10k/100k tests; background scanning and search |
| Search | Unicode case-insensitive, multiple terms; title, file, ID, author, categories and tags |
| Enable/disable | Checkboxes, bulk actions on search results, all/enabled/disabled filters |
| Load order | Move the selected mod with buttons or Alt+arrows; persisted in presets |
| Sorting | Load-order/name view toggle, independent of launch order |
| Presets | Create/update by name, apply, JSON import/export |
| Legacy presets | Full Mod records and compact entries; old sparse loadOrder reconstructed from names and pinned positions |
| Missing mods | Report on preset application; automatic subscription is not implemented |
| Original metadata | `wh3-meta` CLI, GUI export from config, import of v3 games.wh3 and legacy configurations |
| Titles, authors, categories, tags | Imported, stored in binary format and searchable; category editing remains unported |
| Persistence | rkyv WHM1, versioned schema, bytecheck, CRC32, atomic replacement and previous-generation backup |
| Languages | English/Russian switch, persisted binary preference, translated labels, reports, errors and edit menus |
| Steam libraries | Standard paths and libraryfolders.vdf discovery; manual folder selection |
| Scanning | Data, Workshop and custom folders; cycle/path deduplication; per-file error report |
| Pack files | PFH4/PFH5 headers/indexes, including hashed-name PFH5; payloads are not loaded |
| Pack inspection | Virtualized file names, sizes and compression flags |
| Conflicts | Overlapping file paths and missing pack dependencies; not a complete compatibility analysis |
| Workshop | Open a mod's page; Steamworks downloads, updates and subscriptions remain unported |
| WH3 launch | Windows, direct Warhammer3.exe launch with a separate list; real-game verification remains outstanding |
| Builds | Quality workflow plus tag-driven Windows x64 release workflow; GUI/CLI exe, ZIP and checksums retained as artifacts and attached to GitHub Release; unsigned |

## First-stage limitations

- WH3/Steam is supported. Other Total War games and game launch through Linux/Proton
  or macOS are not ported.
- No DB/LOC editor, pack editor, mod merging, compatibility tables, DB-reference
  analysis, game viewers (units, buildings, skills, maps), node flows or game-entity customization.
- No skip-intro/script-logging options or packs generated for those options.
- No multi-selection, drag-and-drop, dual panes, mod context menus, resizable columns,
  thumbnails, category editing, preset deletion or automatic folder watching.
- Automatic before/after load-order rules are not applied. Metadata export preserves
  these rules in JSON and includes a warning. Review the manual order before launching.
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
- Conflict checking does not resolve DB-row winners or prove mod compatibility.
- Native file picker controls and OS diagnostic details follow the system locale.
  Imported names and third-party warnings are preserved verbatim.

## Next stages

1. Verify a real Windows library and WH3 launch; add Steamworks and metadata refresh.
2. Add multi-selection, category editing, dual panes and remaining list interactions.
3. Add load-order rules, DB-conflict analysis and dependencies.
4. Port the original pack/DB/LOC editors and game tools.
