//! Preview image discovery that mirrors the original manager without reading image bytes.
use crate::catalog::{Catalog, Source};
use std::path::{Path, PathBuf};

/// The original manager only recognises these two extensions.
const EXTENSIONS: [&str; 2] = ["png", "jpg"];

fn extension_rank(path: &Path) -> Option<usize> {
    let extension = path.extension()?.to_str()?;
    EXTENSIONS
        .iter()
        .position(|candidate| extension.eq_ignore_ascii_case(candidate))
}

/// Image files of one directory listing, sorted so "first image" is deterministic across
/// platforms (`read_dir` order is not).
pub(super) fn images(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut images: Vec<_> = files
        .iter()
        .filter(|path| extension_rank(path).is_some())
        .cloned()
        .collect();
    images.sort();
    images
}

/// Chooses the thumbnail for `pack` from the images in its own directory.
///
/// - Data: only `<stem>.png`, then `<stem>.jpg`, because the data folder holds many packs.
/// - Workshop: the first `.png`, else the first `.jpg`, because an item folder holds one pack and
///   Steam names its preview image independently of the pack.
/// - Custom: `<stem>.png`/`<stem>.jpg`, else any image next to the pack.
pub(super) fn pick(pack: &Path, source: Source, images: &[PathBuf]) -> Option<PathBuf> {
    let same_stem = || {
        let stem = pack.file_stem()?;
        let mut candidates: Vec<_> = images
            .iter()
            .filter(|image| {
                image.file_stem().is_some_and(|image_stem| {
                    image_stem.eq_ignore_ascii_case(stem) && extension_rank(image).is_some()
                })
            })
            .collect();
        candidates.sort_by_key(|image| extension_rank(image));
        candidates.first().map(|image| (*image).clone())
    };
    let by_extension = || {
        images
            .iter()
            .min_by_key(|image| extension_rank(image))
            .cloned()
    };
    match source {
        Source::Data => same_stem(),
        Source::Workshop => by_extension(),
        Source::Custom => same_stem().or_else(|| images.first().cloned()),
    }
}

/// A Data copy of a Workshop item usually has no image of its own; borrow the image of a
/// non-Data mod with the same pack name from the same scan, preferring the Workshop copy.
pub(super) fn inherit(catalog: &mut Catalog) {
    for index in 0..catalog.mods.len() {
        let item = &catalog.mods[index];
        if item.source != Source::Data || item.thumbnail.is_some() {
            continue;
        }
        let Some(candidates) = catalog.by_name.get(&item.name.to_lowercase()) else {
            continue;
        };
        let donor = candidates
            .iter()
            .filter_map(|&candidate| catalog.mods.get(candidate))
            .filter(|other| other.source != Source::Data)
            .filter_map(|other| Some((other.source, other.thumbnail.as_ref()?)))
            .min_by_key(|(source, _)| *source != Source::Workshop)
            .map(|(_, thumbnail)| thumbnail.clone());
        if let Some(thumbnail) = donor {
            catalog.mods[index].thumbnail = Some(thumbnail);
        }
    }
}
