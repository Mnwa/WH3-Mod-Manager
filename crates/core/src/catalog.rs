mod search;

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Data,
    Workshop,
    #[default]
    Custom,
}

#[derive(Clone, Debug)]
pub struct Mod {
    pub path: PathBuf,
    pub name: Arc<str>,
    pub title: Arc<str>,
    pub workshop_id: Arc<str>,
    pub source: Source,
    pub size: u64,
    pub movie: bool,
    pub dependencies: Vec<String>,
    pub metadata: crate::metadata::Metadata,
    /// Modification time of the pack file. Without Steamworks this is the only local signal for
    /// when a mod changed; Steam usually sets it to the download time, not the author's upload.
    pub modified: Option<SystemTime>,
    /// Preview image path discovered during scanning. The core never reads image bytes.
    pub thumbnail: Option<PathBuf>,
    /// The pack is a symbolic link (e.g. linked into `data` from the Workshop).
    pub linked: bool,
    // Normalize during scanning instead of on every search keystroke.
    search: String,
}

impl Mod {
    pub fn new(
        path: PathBuf,
        title: String,
        workshop_id: String,
        source: Source,
        size: u64,
        movie: bool,
        dependencies: Vec<String>,
    ) -> Self {
        let name: Arc<str> = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
            .into();
        let search = format!("{name}\n{title}\n{workshop_id}").to_lowercase();
        Self {
            path,
            name,
            title: title.into(),
            workshop_id: workshop_id.into(),
            source,
            size,
            movie,
            dependencies,
            search,
            metadata: Default::default(),
            modified: None,
            thumbnail: None,
            linked: false,
        }
    }

    pub fn with_link(mut self, linked: bool) -> Self {
        self.linked = linked;
        self
    }

    /// Builder-style setter so existing `Mod::new` call sites stay unchanged.
    pub fn with_modified(mut self, modified: Option<SystemTime>) -> Self {
        self.modified = modified;
        self
    }

    /// Builder-style setter so existing `Mod::new` call sites stay unchanged.
    pub fn with_thumbnail(mut self, thumbnail: Option<PathBuf>) -> Self {
        self.thumbnail = thumbnail;
        self
    }

    /// True when the pack last changed strictly before the game update, as in the original
    /// manager's "possibly outdated" warning. Every source is checked: a Data or Custom pack that
    /// predates a patch is as likely to be stale as a Workshop one. A mod without a known
    /// modification time is never flagged.
    pub fn is_outdated(&self, game_updated: SystemTime) -> bool {
        self.modified
            .is_some_and(|modified| modified < game_updated)
    }

    pub(crate) fn rebuild_search(&mut self) {
        self.search = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            self.name,
            self.title,
            self.workshop_id,
            self.metadata.author,
            self.metadata.categories.join(" "),
            self.metadata.tags.join(" ")
        )
        .to_lowercase();
    }
}

/// `Clone` lets the UI copy-on-write a shared snapshot for a rare metadata edit.
#[derive(Clone, Default)]
pub struct Catalog {
    pub mods: Vec<Mod>,
    pub by_name: HashMap<String, Vec<usize>>,
}

impl Catalog {
    /// Recompute search text after bulk field edits (titles from Workshop, etc.).
    pub fn rebuild_search_all(&mut self) {
        self.mods.iter_mut().for_each(Mod::rebuild_search);
    }

    /// Edit one mod's user metadata and keep its search text in sync.
    pub fn update_metadata(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut crate::metadata::Metadata),
    ) {
        if let Some(item) = self.mods.get_mut(index) {
            edit(&mut item.metadata);
            item.rebuild_search();
        }
    }

    pub fn new(mods: Vec<Mod>) -> Self {
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::with_capacity(mods.len());
        for (index, item) in mods.iter().enumerate() {
            by_name
                .entry(item.name.to_lowercase())
                .or_default()
                .push(index);
        }
        Self { mods, by_name }
    }

    /// One flag per catalog index (same length as `mods`); see [`Mod::is_outdated`].
    pub fn outdated(&self, game_updated: SystemTime) -> Vec<bool> {
        self.mods
            .iter()
            .map(|item| item.is_outdated(game_updated))
            .collect()
    }

    pub fn demo(count: usize) -> Self {
        // A fixed epoch (2026-01-01 UTC) keeps demo screenshots and tests deterministic.
        let newest = SystemTime::UNIX_EPOCH + Duration::from_secs(1_767_225_600);
        Self::new(
            (0..count)
                .map(|i| {
                    Mod::new(
                        PathBuf::from(format!("demo/mod_{i:06}.pack")),
                        format!(
                            "{} · расширение {:06}",
                            [
                                "Immortal Empires",
                                "Гномы Караза",
                                "SFO Grimhammer",
                                "Кислев",
                                "Campaign overhaul"
                            ][i % 5],
                            i
                        ),
                        (3_000_000_000u64 + i as u64).to_string(),
                        Source::Workshop,
                        (i as u64 + 1) * 4096,
                        false,
                        vec![],
                    )
                    .with_modified(
                        newest.checked_sub(Duration::from_secs((i as u64 % 10_000) * 3_600)),
                    )
                })
                .collect(),
        )
    }
}
