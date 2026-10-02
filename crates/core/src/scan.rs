use crate::{
    Error, Result,
    catalog::{Catalog, Mod, Source},
    pack,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Scan {
    pub catalog: Catalog,
    pub warnings: Vec<crate::localization::Message>,
}

pub fn scan(roots: &[(PathBuf, Source)], cancelled: &AtomicBool) -> Result<Scan> {
    let mut mods = Vec::new();
    let mut warnings = Vec::new();
    let mut visited = HashSet::new();
    let mut paths = HashSet::new();
    for (root, source) in roots {
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            if cancelled.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let canonical = match fs::canonicalize(&directory) {
                Ok(path) => path,
                Err(e) => {
                    warnings.push(crate::error::io(&directory, e).message());
                    continue;
                }
            };
            if !visited.insert(canonical) {
                continue;
            }
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(e) => {
                    warnings.push(crate::error::io(&directory, e).message());
                    continue;
                }
            };
            for entry in entries {
                if cancelled.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        warnings.push(crate::message!("File error: {}", "Ошибка файла: {}", e));
                        continue;
                    }
                };
                let path = entry.path();
                if path.is_dir() {
                    if *source != Source::Data {
                        pending.push(path);
                    }
                    continue;
                }
                if !path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pack"))
                {
                    continue;
                }
                let canonical = match fs::canonicalize(&path) {
                    Ok(path) => path,
                    Err(e) => {
                        warnings.push(crate::message!("File error: {}", "Ошибка файла: {}", e));
                        continue;
                    }
                };
                if !paths.insert(canonical) {
                    continue;
                }
                match read_mod(&path, *source) {
                    Ok(Some(item)) => mods.push(item),
                    Ok(None) => {}
                    Err(e) => warnings.push(e.message()),
                }
            }
        }
    }
    mods.sort_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
    Ok(Scan {
        catalog: Catalog::new(mods),
        warnings,
    })
}

fn read_mod(path: &Path, source: Source) -> Result<Option<Mod>> {
    let header = pack::header(path)?;
    let kind = header.flags & 0xf;
    if source == Source::Data && kind < 3 {
        return Ok(None);
    }
    let workshop_id = if source == Source::Workshop {
        path.ancestors()
            .skip(1)
            .filter_map(Path::file_name)
            .filter_map(|name| name.to_str())
            .find(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or_default()
            .to_owned()
    } else {
        String::new()
    };
    let title = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace('_', " ");
    Ok(Some(Mod::new(
        path.to_owned(),
        title,
        workshop_id,
        source,
        header.pack_size,
        kind == 4,
        header.dependencies,
    )))
}
