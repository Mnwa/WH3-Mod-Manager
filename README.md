# WH3 Mod Manager

A fast mod manager for **Total War: Warhammer III** on Steam. It is a native Windows
rewrite of [Shazbot's WH3 Mod Manager](https://github.com/Shazbot/WH3-Mod-Manager):
the same everyday features, without Electron, and it can import everything you set up
in the original. The interface is available in English and Russian.

Features not present in the CA launcher:

- instant even with hundreds or thousands of mods, with thumbnails, authors and update dates
- drag-and-drop load order, "load before/after" rules and pinned positions
- presets, categories, hidden and always-enabled mods, search (including `/regex/`)
- options to skip intro movies, enable script logging, auto-start a custom battle
  and let units be picked as generals
- a compatibility checker that shows which mod wins overwritten files and DB rows,
  missing requirements and start-position conflicts
- update, subscribe and unsubscribe Workshop mods, install missing requirements and
  import Steam collections directly from the manager
- **Continue** your latest campaign, load any save, or enable exactly the mods a save used
- imports presets, load order, categories, rules and options from the original manager
- updates itself from GitHub Releases

![The mod list with thumbnails, badges and the compatibility report](docs/screenshot.png)

## Download and install

1. Open the [latest release](https://github.com/Mnwa/WH3-Mod-Manager/releases/latest).
2. Under **Assets**, download `WH3-Mod-Manager-…-windows-x64.zip`.
3. Unpack the whole archive into any folder, for example `Documents\WH3 Mod Manager`.
   Keep `steam_api64.dll` next to `wh3-mod-manager.exe`: Workshop features need it.
4. Run `wh3-mod-manager.exe`.

Windows may show "Windows protected your PC" because the program is not signed.
Click **More info → Run anyway**. You only need 64-bit Windows 10 or 11 and Steam
with Total War: Warhammer III; nothing else has to be installed.

New versions are found automatically: when a green **Update** button appears in the
top bar, click it, then **Restart**. Your mod list is saved first.

## First start

- The manager finds the game in your Steam libraries. If it does not, click
  **Game folder** and choose the folder that contains `Warhammer3.exe`.
- **Coming from the original manager?** Open **Options → Import metadata or
  config.json…** and choose its `config.json`, usually
  `%APPDATA%\wh3mm\config.json`. Your enabled mods, load order, presets, categories,
  rules, hidden and always-enabled mods and start options are imported; the original
  file is not changed. Then click **Save**.
- Switch the language with the **Русский / English** button in the top bar.

## Everyday use

- **Enable mods** with the checkboxes. Clicking a column header sorts the table; this
  never changes the order the game loads mods in.
- **Load order**: drag a row by its number, or use Top/Up/Down/Bottom
  (`Alt+↑/↓`, `Alt+Home/End`). Mods higher in the list win conflicts. A mod you moved
  by hand stays where you put it. Right-click a mod and choose **Load before…** or
  **Load after…** to add a rule that keeps two mods in the right order; the
  **Rules** button lists your rules and the ones mods ship with.
- **Select several mods** with `Ctrl+click`, `Shift+click` or `Ctrl+A`, then press
  `Space` or right-click to enable, disable, move or hide them together.
- **Right-click a mod** for its Workshop page, Steam, the folder on disk, categories,
  **Keep always enabled**, **Hide from list**, **Update from Workshop** and
  **Unsubscribe**.
- **Presets**: type a name and click **Save preset**. Click a preset to apply it; its
  **…** menu can add or remove its mods, replace it with the current list or delete
  it. "On Last Game Launch" is refreshed on every launch.
- **Check compatibility** lists overwritten files, colliding DB rows (with the mod
  that wins), missing dependencies and conflicting start positions, and marks the
  affected rows. **Get required mods** subscribes to what is missing and enables it
  once Steam has downloaded it.
- **Workshop** in the top bar refreshes mod data, updates outdated mods, re-downloads
  enabled ones and imports a Steam collection link. Steam must be running.
- **Options** holds the game start parameters (keep them identical with friends for
  multiplayer) and **Close manager on Play**.
- **Play** starts the game with your mods; **Continue** loads your newest campaign
  save, and the folder button next to it lists recent saves.

Row badges: lock — always enabled; film — movie pack; clock — older than the last game
update and overwrites game files; download arrow — an update is on the Workshop;
triangle — conflicts; circle — missing requirements. Hover a badge for details.

Keyboard: `Ctrl+F` search, `Ctrl+S` save, `Space` enable/disable, `Esc` close the report.

## Where your data is stored

The mod list, presets and settings are kept in `%APPDATA%\wh3-mod-manager-rust`.
Every save keeps the previous version as `library.whmm.bak`. To keep everything in
another folder (for example a portable copy), set the `WH3MM_HOME` environment
variable to that folder.

The manager writes its own mod list, `wh3_rust_mods.txt`, into the game folder, so it
never overwrites the original manager's `used_mods.txt`. Files generated for the start
options live in the data folder above, never in the game's `data` folder.

## Status and differences from the original

The mod-management part of the original is ported. The pack/DB editors, game data
viewers and node flows are not. [MIGRATION.md](docs/MIGRATION.md) lists what is
ported and what still differs.

Found a problem? [Open an issue](https://github.com/Mnwa/WH3-Mod-Manager/issues).
Developers: see [DEVELOPMENT.md](DEVELOPMENT.md).

## License

MIT. Based on the original WH3 Mod Manager by Shazbot. The bundled DB schema derives
from [RPFM's schemas](https://github.com/Frodo45127/rpfm-schemas) (MIT); see
[crates/core/assets/NOTICE](crates/core/assets/NOTICE).
