use crate::{Error, Result, catalog::Catalog, pack, storage};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

mod process;
mod start_pack;

pub use process::{arguments, start};
pub use start_pack::{
    AUTO_START_CUSTOM_BATTLE, AUTO_START_CUSTOM_BATTLE_SCRIPT, EMPTY_MOVIE, INTRO_MOVIES, Options,
    SCRIPT_LOGGING, TEMP_PACK, start_pack_files,
};

pub const MOD_LIST: &str = "wh3_rust_mods.txt";

fn quote(value: &str) -> Result<String> {
    if value.contains(['"', '\n', '\r', '\0', ';']) {
        return Err(Error::Invalid(crate::message!(
            "Invalid characters in mod path",
            "Недопустимые символы в пути мода"
        )));
    }
    Ok(format!("\"{value}\""))
}

/// Folder for packs the manager generates before a launch. It lives next to the
/// library so generated files never land in the game or Workshop folders.
pub fn temp_pack_dir() -> Result<PathBuf> {
    let settings = storage::settings_path()?;
    let parent = settings.parent().ok_or_else(|| {
        Error::Invalid(crate::message!(
            "Configuration folder not found",
            "Не найдена папка настроек"
        ))
    })?;
    Ok(parent.join("temp_packs"))
}

/// Builds the mod list. When any start option is on, the generated pack is
/// appended last, as the original manager does, so the list ends with
/// `add_working_directory "<temp_dir>";` and `mod "!!!!out.pack";`.
pub fn script(
    catalog: &Catalog,
    order: &[usize],
    enabled: &HashSet<usize>,
    options: &Options,
    temp_dir: &Path,
) -> Result<String> {
    let mut directories = HashSet::new();
    let mut names = HashSet::new();
    if options.any() {
        // A user pack with the generated name would shadow or be shadowed by it.
        names.insert(TEMP_PACK.to_lowercase());
    }
    let mut lines = Vec::new();
    let mut mods = Vec::new();
    for &index in order.iter().filter(|i| enabled.contains(i)) {
        let item = catalog.mods.get(index).ok_or_else(|| {
            Error::Invalid(crate::message!(
                "Mod is missing from the catalog",
                "Мод отсутствует в каталоге"
            ))
        })?;
        if !names.insert(item.name.to_lowercase()) {
            return Err(Error::Invalid(crate::message!(
                "Two enabled mods have the same name: {}",
                "Два включённых мода имеют имя {}",
                item.name
            )));
        }
        let parent = item.path.parent().ok_or_else(|| {
            Error::Invalid(crate::message!(
                "Mod has no parent folder",
                "У мода нет папки"
            ))
        })?;
        if directories.insert(parent) {
            lines.push(format!(
                "add_working_directory {};",
                quote(&parent.to_string_lossy())?
            ));
        }
        mods.push(format!("mod {};", quote(&item.name)?));
    }
    lines.extend(mods);
    if options.any() {
        lines.push(format!(
            "add_working_directory {};",
            quote(&temp_dir.to_string_lossy())?
        ));
        lines.push(format!("mod {};", quote(TEMP_PACK)?));
    }
    Ok(lines.join("\n"))
}

pub fn prepare(
    game: &Path,
    catalog: &Catalog,
    order: &[usize],
    enabled: &HashSet<usize>,
    options: &Options,
    temp_dir: &Path,
) -> Result<()> {
    if !game.join("Warhammer3.exe").is_file() {
        return Err(Error::Invalid(crate::message!(
            "Select the folder containing Warhammer3.exe",
            "Выберите папку с Warhammer3.exe"
        )));
    }
    for &index in enabled {
        let item = catalog
            .mods
            .get(index)
            .ok_or_else(|| Error::Invalid(crate::message!("Mod is missing", "Мод отсутствует")))?;
        if !item.path.is_file() {
            return Err(Error::Invalid(crate::message!(
                "Mod file not found: {}",
                "Мод не найден: {}",
                item.path.display()
            )));
        }
    }
    for (index, item) in catalog.mods.iter().enumerate() {
        if item.movie && item.source == crate::catalog::Source::Data && !enabled.contains(&index) {
            return Err(Error::Invalid(crate::message!(
                "{} is a movie pack in data and loads automatically. Move it out of data to disable it.",
                "{} — movie pack в data: игра загружает его автоматически. Переместите файл из data, чтобы отключить.",
                item.name
            )));
        }
    }
    // Validate the whole list before writing anything.
    let script = script(catalog, order, enabled, options, temp_dir)?;
    let generals = if options.make_units_generals {
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        Some(crate::db::make_units_generals_table(
            &game.join("data"),
            catalog,
            order,
            enabled,
            &cancelled,
        )?)
    } else {
        None
    };
    write_start_pack(options, generals, temp_dir)?;
    storage::atomic_write(&game.join(MOD_LIST), script.as_bytes())
}

fn write_start_pack(options: &Options, generals: Option<Vec<u8>>, temp_dir: &Path) -> Result<()> {
    let path = temp_dir.join(TEMP_PACK);
    if !options.any() {
        // The mod list no longer references the pack, so a removal failure
        // (for example a game instance still holding it) must not block launch.
        let _ = std::fs::remove_file(path);
        return Ok(());
    }
    let files = start_pack_files(options);
    // `db\…` sorts before `movies\…` and `script\…`, keeping the index ascending.
    let entries: Vec<(&str, &[u8])> = generals
        .as_deref()
        .map(|table| (crate::db::UNITS_GENERALS_PATH, table))
        .into_iter()
        .chain(
            files
                .iter()
                .map(|(name, content)| (*name, content.as_slice())),
        )
        .collect();
    pack::write(&path, &entries)
}
