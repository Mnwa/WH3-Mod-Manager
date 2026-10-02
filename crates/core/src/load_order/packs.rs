//! Collect rules that mods ship in `whmm\load_order.whmm`.
use super::{Rule, is_rules_file_path, parse_rules_bytes};
use crate::{Error, Result, catalog::Catalog, localization::Message, pack};
use std::{
    io::{Read, Seek, SeekFrom},
    sync::atomic::{AtomicBool, Ordering},
};

/// Rules files are a few lines; anything larger is not one.
const MAX_RULES_FILE: u32 = 1024 * 1024;

fn read_entry(path: &std::path::Path, entry: &pack::PackedFile) -> Result<Vec<u8>> {
    if entry.compressed {
        return pack::Reader::open(path)?.read(entry);
    }
    let mut file = std::fs::File::open(path).map_err(|e| crate::error::io(path, e))?;
    file.seek(SeekFrom::Start(entry.offset))
        .map_err(|e| crate::error::io(path, e))?;
    let mut bytes = vec![0; entry.size as usize];
    file.read_exact(&mut bytes)
        .map_err(|e| crate::error::io(path, e))?;
    Ok(bytes)
}

/// Every pack rule in the catalog. Only pack indexes are scanned; payloads are
/// read just for packs that contain a rules file.
pub fn pack_rules(catalog: &Catalog, cancelled: &AtomicBool) -> Result<(Vec<Rule>, Vec<Message>)> {
    let (mut rules, mut warnings) = (Vec::new(), Vec::new());
    for item in &catalog.mods {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let Ok(index) = pack::index(&item.path) else {
            continue;
        };
        let Some(entry) = index.iter().find(|entry| is_rules_file_path(&entry.name)) else {
            continue;
        };
        if entry.size > MAX_RULES_FILE {
            warnings.push(crate::message!(
                "{}: load-order rules file is too large",
                "{}: файл правил порядка слишком большой",
                item.name
            ));
            continue;
        }
        match read_entry(&item.path, entry) {
            Ok(bytes) => {
                let (found, problems) = parse_rules_bytes(&item.name, &bytes);
                rules.extend(found);
                warnings.extend(problems);
            }
            Err(error) => warnings.push(error.message().prefixed(&format!("{}: ", item.name))),
        }
    }
    Ok((rules, warnings))
}
