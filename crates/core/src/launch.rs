use crate::{Error, Result, catalog::Catalog, storage};
use std::{collections::HashSet, path::Path};

pub const MOD_LIST: &str = "wh3_rust_mods.txt";

fn quote(value: &str) -> Result<String> {
    if value.contains(['"', '\n', '\r', '\0', ';']) {
        return Err(Error::Invalid("Недопустимые символы в пути мода".into()));
    }
    Ok(format!("\"{value}\""))
}

pub fn script(catalog: &Catalog, order: &[usize], enabled: &HashSet<usize>) -> Result<String> {
    let mut directories = HashSet::new();
    let mut names = HashSet::new();
    let mut lines = Vec::new();
    let mut mods = Vec::new();
    for &index in order.iter().filter(|i| enabled.contains(i)) {
        let item = catalog
            .mods
            .get(index)
            .ok_or_else(|| Error::Invalid("Мод отсутствует в каталоге".into()))?;
        if !names.insert(item.name.to_lowercase()) {
            return Err(Error::Invalid(format!(
                "Два включённых мода имеют имя {}",
                item.name
            )));
        }
        let parent = item
            .path
            .parent()
            .ok_or_else(|| Error::Invalid("У мода нет папки".into()))?;
        if directories.insert(parent) {
            lines.push(format!(
                "add_working_directory {};",
                quote(&parent.to_string_lossy())?
            ));
        }
        mods.push(format!("mod {};", quote(&item.name)?));
    }
    lines.extend(mods);
    Ok(lines.join("\n"))
}

pub fn prepare(
    game: &Path,
    catalog: &Catalog,
    order: &[usize],
    enabled: &HashSet<usize>,
) -> Result<()> {
    if !game.join("Warhammer3.exe").is_file() {
        return Err(Error::Invalid("Выберите папку с Warhammer3.exe".into()));
    }
    for &index in enabled {
        let item = catalog
            .mods
            .get(index)
            .ok_or_else(|| Error::Invalid("Мод отсутствует".into()))?;
        if !item.path.is_file() {
            return Err(Error::Invalid(format!(
                "Мод не найден: {}",
                item.path.display()
            )));
        }
    }
    for (index, item) in catalog.mods.iter().enumerate() {
        if item.movie && item.source == crate::catalog::Source::Data && !enabled.contains(&index) {
            return Err(Error::Invalid(format!(
                "{} — movie pack в data: игра загружает его автоматически. Переместите файл из data, чтобы отключить.",
                item.name
            )));
        }
    }
    storage::atomic_write(
        &game.join(MOD_LIST),
        script(catalog, order, enabled)?.as_bytes(),
    )
}

#[cfg(target_os = "windows")]
pub fn start(game: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(game.join("Warhammer3.exe"))
        .current_dir(game)
        .arg(format!("{MOD_LIST};"))
        .creation_flags(0x00000008)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| crate::error::io(game, e))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn start(_: &Path) -> Result<()> {
    Err(Error::Invalid(
        "Запуск игры доступен в Windows-сборке".into(),
    ))
}
