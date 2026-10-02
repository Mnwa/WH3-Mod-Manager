use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

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
    // Нормализация выполняется при сканировании, а не на каждый символ поиска.
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
        }
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

#[derive(Default)]
pub struct Catalog {
    pub mods: Vec<Mod>,
    pub by_name: HashMap<String, Vec<usize>>,
}

impl Catalog {
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

    pub fn query(&self, query: &str, order: &[usize]) -> Vec<usize> {
        let query = query.to_lowercase();
        let terms: Vec<_> = query.split_whitespace().collect();
        order
            .iter()
            .copied()
            .filter(|&index| {
                self.mods
                    .get(index)
                    .is_some_and(|item| terms.iter().all(|term| item.search.contains(term)))
            })
            .collect()
    }

    pub fn demo(count: usize) -> Self {
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
                })
                .collect(),
        )
    }
}
