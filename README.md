# WH3 Mod Manager · Rust

A native Total War: Warhammer III (Steam) mod manager built with Rust and GPUI Kit.
This is the first stage of the [Shazbot/WH3-Mod-Manager](https://github.com/Shazbot/WH3-Mod-Manager)
port. Implemented features and remaining differences are tracked in [MIGRATION.md](docs/MIGRATION.md).

## Running

Use the latest **stable Rust**. The compiler version is not pinned.
Windows builds require Visual Studio Build Tools with C++ and the Windows SDK, plus CMake.
macOS builds require Command Line Tools.

```sh
cargo run -p wh3-mod-manager --release --locked
# Explore a large library without installing the game:
cargo run -p wh3-mod-manager --release --locked -- --demo=100000
```

Choose the folder containing `Warhammer3.exe`. The manager scans `data` and the
adjacent Steam Workshop folder `1142710`; additional mod folders can be added manually.
Use checkboxes to enable mods and select a row to change its load order. The **Enable**
and **Disable** buttons affect all current search results. Search covers titles, pack
names, Workshop IDs, imported authors, tags and categories. Sorting the table does not
change the launch order.

The interface supports **English and Russian**. Use the language button in the top
bar to switch immediately; the choice is saved automatically. English is the initial
default. Demo mode does not change saved preferences. Labels, tooltips, reports,
application errors and input edit menus follow the selected language. Native file
picker controls and operating-system error details follow the system language;
mod titles, preset names and imported user content retain their original text.

`Ctrl/Cmd+F` focuses search; `Ctrl/Cmd+S` saves; `Alt+Up/Down` moves the selected mod;
`Esc` closes the report. Unsaved library changes are saved when closing the window.
If saving fails, the window stays open and offers a choice to resolve the error or
close without saving.

**Play** is available on Windows. It writes a separate `wh3_rust_mods.txt`, preserving
the original manager's `used_mods.txt`. Launching a real game still requires manual
Windows verification; automated tests cover script contents and launch arguments.

## Migrating original metadata

Save settings in the original manager first. Its `config.json` is in the Electron
user-data directory, or beside the executable for a portable installation.

In the Rust manager:

1. Choose **Original metadata…**, select the original `config.json`, and save `wh3-metadata.json`.
2. Choose **Import metadata** and select the exported file, or import `config.json` directly.
3. Review missing mods, load order and enabled states, then save the library.

The exporter also works without the GUI, Steam or Node.js:

```sh
cargo run -p wh3-core --bin wh3-meta --locked -- export config.json wh3-metadata.json
# From the Windows distribution:
wh3-meta.exe export config.json wh3-metadata.json
wh3-meta.exe --lang=ru export config.json wh3-metadata.json
```

The source configuration is never modified. Export includes saved titles, authors,
categories, tags, Workshop IDs, dependencies and presets when present in the source.
Missing data is neither invented nor downloaded.

Internal persistence uses the **rkyv** binary `library.whmm` format. JSON is reserved
for interchange. See [STORAGE.md](docs/STORAGE.md) for the format, backup and recovery.

## Quality checks and Windows EXE

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo bench -p wh3-core --bench catalog --locked
```

On macOS, the `visual` test uses the real Metal renderer, writes screenshots into
`target/visual`, checks both interface languages and validates virtualization with
1,000 / 10,000 / 100,000 mods. This specific test is explicitly skipped elsewhere.

GitHub Actions checks the core on Linux and the entire workspace on Windows/macOS,
then builds a Windows x64 release. The `WH3-Mod-Manager-windows-x64` artifact contains
the GUI executable, `wh3-meta.exe`, a distribution ZIP and SHA-256 checksums.
Executables are unsigned.

## Tagged releases

The separate `.github/workflows/release.yml` runs on a pushed `v*` tag or a published
GitHub Release. It checks out that exact tag, sets the workspace and first-party
lockfile versions from it, runs Windows quality checks, and builds with latest stable
Rust. Dependency versions stay locked. Use SemVer tags such as `v0.1.0` or `v0.2.0-rc.1`.

The workflow retains an Actions artifact and attaches these files to the tag's Release:

- `wh3-mod-manager.exe` and `wh3-meta.exe` (Windows x64).
- `WH3-Mod-Manager-<tag>-windows-x64.zip` with documentation and the license.
- `SHA256SUMS.txt` covering both executables and the ZIP.

If no Release exists, the workflow creates one with generated notes. Prerelease tags
create prereleases. Reruns replace matching assets while preserving existing release
notes. Publishing uses the built-in `GITHUB_TOKEN`; no additional secret is needed.
The tag must point to a commit containing the release workflow and scripts.

For a manual rebuild of an existing tag:

```sh
gh workflow run release.yml -f tag=v0.1.0
# Validate a build without publishing or replacing release assets:
gh workflow run release.yml -f tag=v0.1.0 -f publish=false
```

Manual dispatch becomes available once the workflow is on the default branch. A tag
push and a separately published Release can each queue a run; uploads for the same
tag are serialized. Releases created by this workflow's token do not recursively
trigger another run.

## Repository layout

- `crates/core`: catalog, Steam paths, pack indexes, presets, metadata, binary persistence and launch logic.
- `crates/desktop`: GPUI components, background jobs and virtualized lists.
- `docs/PERFORMANCE.md`: measured results and validation boundaries.
- `AGENTS.md`: development rules adapted from `cr-chat-desktop`.

Copyright 2026 Mikhail Panfilov (Mnwa). The original Shazbot project (MIT) provides
the format and interaction reference; GPUI and binary storage practices follow
`cr-chat-desktop`. The original copyright notice is preserved in [LICENSE](LICENSE).
