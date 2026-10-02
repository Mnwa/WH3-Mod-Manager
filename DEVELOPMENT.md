# Development

Rules for contributors (and coding agents) are in [AGENTS.md](AGENTS.md). Ported
features and known differences from the original manager are tracked in
[docs/MIGRATION.md](docs/MIGRATION.md).

## Layout

- `crates/core` (`wh3-core`): catalog, scanning, pack indexes and payload reading,
  DB tables and schema, compatibility analysis, load-order rules, presets, metadata
  import/export, saves, launch and start pack, binary persistence, Steam Workshop
  worker and self-update. No GUI dependencies.
- `crates/desktop` (`wh3-mod-manager`): the GPUI application. `manager/` is the mod
  management screen, `update/` the self-update slice, `assets.rs` the icon source.
- `crates/core/assets`: the embedded WH3 DB schema and its `NOTICE`.
- `crates/desktop/assets`: `logo.svg` (the source of truth for the logo, shown in the
  app and the READMEs), the generated `app.ico` and `windows/app.rc`, which `build.rs`
  compiles into Windows builds together with version information from the package
  version. After changing the logo, regenerate the icon with
  `cargo run -p wh3-mod-manager --example render_icon --locked`.
- `docs/`: `MIGRATION.md` and the README screenshots (English and Russian).

## Building and running

Use the latest **stable Rust**; the compiler version is not pinned. Windows builds
need Visual Studio Build Tools (C++), the Windows SDK and CMake. macOS builds need the
Command Line Tools. Linux builds need the GPUI system libraries (see the
`Install Linux dependencies` step in `.github/workflows/ci.yml`).

```sh
cargo run -p wh3-mod-manager --release --locked
# A synthetic library; nothing on disk is read or written:
cargo run -p wh3-mod-manager --release --locked -- --demo=100000
```

`WH3MM_HOME=<folder>` keeps the library, preferences and generated packs in that
folder instead of the user configuration directory. Use it to test against a fake
game install or a copy of a real library without touching the real one. Steam paths
and campaign saves are still discovered normally (read-only).

### Windows GUI from WSL 2

Build and run with the Windows toolchain through interop; do not cross-compile.

```sh
WIN_HOME=$(wslpath "$(cmd.exe /c 'echo %USERPROFILE%' 2>/dev/null | tr -d '\r')")
export CARGO_TARGET_DIR="$WIN_HOME/AppData/Local/wh3-target"
export WSLENV=CARGO_TARGET_DIR/p
"$WIN_HOME/.cargo/bin/cargo.exe" build -p wh3-mod-manager --locked
"$CARGO_TARGET_DIR/debug/wh3-mod-manager.exe" --demo=500 &
```

Keep the target directory on the Windows disk. Steam features in a development
build need `steam_api64.dll` next to the executable: copy it from
`$CARGO_TARGET_DIR/debug/build/steamworks-sys-*/out/`.

## Architecture notes

- **Background work.** Filesystem access, pack and DB parsing, conflict analysis,
  searching, sorting, rule application and every Steam call run on GPUI's background
  executor; tasks are owned by the view and guarded by generation counters.
- **Steam Workshop.** Initialising Steamworks with the game's app id makes Steam
  treat the process as the running game, so the GUI never does it. Each request
  (`Details`, `State`, `Subscribe`, `Unsubscribe`, `Download`, `Subscribed`) spawns
  `wh3-mod-manager.exe --steam-worker`, which answers one JSON request from stdin on
  stdout after a marker line (Steamworks prints its own diagnostics to stdout) and
  exits. The executable delay-loads `steam_api64.dll` (`crates/desktop/build.rs`), so
  the GUI starts without the DLL; the release archive ships it next to the executable.
  You can drive the worker by hand:
  `echo '"Subscribed"' | wh3-mod-manager.exe --steam-worker`.
- **Load order.** The order is an explicit list. Mods moved by hand are pinned
  (stored as `loadOrder` in presets, like the original); user rules and rules shipped
  in `whmm\load_order.whmm` move only unpinned mods and are applied after changes and
  again right before launch. Presets from the original (`version: 2` with pins) are
  ordered with its `sortByNameAndLoadOrder`; presets written here use `version: 3`,
  whose list order is the load order.
- **Compatibility.** File conflicts come from pack indexes only. Tables shared by
  several mods are then parsed with the bundled schema to find colliding row keys.
- **Launch.** `Play` writes `wh3_rust_mods.txt` and, for start options, a generated
  `!!!!out.pack` in the data folder's `temp_packs`, loaded last.

## Persistence

`library.whmm` uses rkyv records framed as magic, payload length and CRC32, validated
with bytecheck. WHM4 is current; WHM1–WHM3 are decode-only and migrate on the next
save, and each version rejects option bits it never had. Every save is atomic and
keeps the previous generation as `library.whmm.bak`; a damaged library is reported and
never replaced by an empty state. Frozen fixtures for every version live in
`crates/core/tests/fixtures` and must not change. UI preferences (language, layout,
row size, grouping, column widths) use the separate fixed-length WHP2 record in
`preferences.whmp`; WHP1 (language only) is decoded for migration. JSON is only used
for interchange.

## Game folder writes

Besides `wh3_rust_mods.txt`, the manager writes into the game folder only when the user
asks: copies or links in `data` (`wh3_core::data_folder`, never overwriting, deleting
only named top-level `.pack` files after confirmation) and the Workshop staging folder
`whmm_copied_mods` (`wh3_core::staging`). Test these against a fake Steam library:
create `<dir>/steamapps/common/WH3` with a stand-in `Warhammer3.exe` and `data`, put a
few packs under `<dir>/steamapps/workshop/content/1142710/<id>/`, point a library in a
throw-away `WH3MM_HOME` at it, and use a long-running stand-in process named
`Warhammer3.exe` to exercise game detection, priority and cleanup after exit.

## Metadata from the original manager

The GUI imports the original `config.json` directly. A source-only CLI exports the
same data without Steam, Node.js or the GUI (it is not attached to releases):

```sh
cargo run -p wh3-core --bin wh3-meta --locked -- export config.json wh3-metadata.json
cargo run -p wh3-core --bin wh3-meta --locked -- --lang=ru export config.json wh3-metadata.json
```

The source configuration is never modified.

## Quality checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo bench -p wh3-core --bench catalog --locked
```

On macOS the `visual` test renders through Metal, writes screenshots into
`target/visual`, checks both languages and virtualization with 1,000 / 10,000 /
100,000 mods; elsewhere it is skipped. `crates/core/tests/real_data.rs` holds
`#[ignore]` tests against a real WH3 install (vanilla `db.pack` and Workshop mods):

```sh
cargo test -p wh3-core --release --test real_data -- --ignored --nocapture
```

## CI, releases and changelogs

- `ci.yml` runs on pushes to `main` and pull requests targeting `main`: formatting,
  Clippy, tests, doc tests, release-version handling and the catalog benchmark on
  Ubuntu. Windows and macOS GUI checks are run locally. After successful pushes,
  Sampo prepares a draft release PR with synchronized versions and changelogs.
- `release.yml` runs when a GitHub Release is published. It checks out the release
  tag, builds the Windows x64 executable with the latest stable Rust and attaches:
  - `wh3-mod-manager-windows-x64.exe` (unsigned; used by the self-updater, which
    installs it as `wh3-mod-manager.exe`),
  - `steam_api64.dll` (Steamworks redistributable, refreshed by the self-updater),
  - `WH3-Mod-Manager-<tag>-windows-x64.zip` with both, the documentation and the license,
  - `SHA256SUMS.txt` covering all of the above.

Add user-visible changes as English [Sampo changesets](.sampo/README.md). Merge the
release PR, then publish a GitHub Release with the matching `v<version>` tag. To
rebuild assets of an existing release: `gh workflow run release.yml -f tag=v0.1.0`.
Tag pushes alone do not trigger `release.yml`.
