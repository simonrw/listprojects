use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use skim::{SkimItem, SkimItemSender};

use crate::SelectablePath;

fn cache_filename() -> PathBuf {
    let cache_dir = dirs::cache_dir()
        .expect("could not compute cache dir")
        .join("listprojects");
    if !cache_dir.is_dir() {
        std::fs::create_dir_all(&cache_dir).expect("creating cache dir");
    }
    cache_dir.join("cache.txt")
}

#[derive(Clone)]
pub struct Cache {
    items: HashSet<PathBuf>,
}

impl Cache {
    pub fn new() -> Self {
        let cache_filename = cache_filename();
        let items = if cache_filename.is_file() {
            let contents =
                std::fs::read_to_string(&cache_filename).expect("reading cache contents");
            contents
                .lines()
                .map(PathBuf::from)
                .collect::<HashSet<PathBuf>>()
        } else {
            HashSet::new()
        };

        Cache { items }
    }

    pub fn prepopulate_with(&self, tx: SkimItemSender) {
        // Implementation for prepopulating the cache with project names
        eprintln!("prepopulating cache with {} items", self.items.len());
        for p in &self.items {
            let item: Arc<dyn SkimItem> = Arc::new(SelectablePath { path: p.clone() });
            let _ = tx.send(item);
        }
    }

    /// Add an item to the cache if not already present, and return true if the cache was updated
    pub fn add_to_cache(&mut self, value: impl Into<PathBuf>) -> bool {
        self.items.insert(value.into())
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        // Implementation for saving the cache to disk
        self.save_items(self.items.iter().cloned(), cache_filename());
        Ok(())
    }

    fn save_items(&self, items: impl Iterator<Item = PathBuf>, output_path: impl AsRef<Path>) {
        let items: Vec<_> = items.collect();
        eprintln!("saving {} items", items.len());
        let mut f = std::fs::File::create(output_path).expect("creating cache file");
        for item in items {
            writeln!(f, "{}", item.display()).expect("writing item to cache file");
        }
        f.flush().expect("flushing cache file");
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        eprintln!("persisting cache to disk");
        self.save().expect("Failed to save cache");
    }
}
