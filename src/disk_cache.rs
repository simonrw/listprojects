use std::{
    collections::HashMap,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use skim::{SkimItem, SkimItemSender};

use crate::SelectablePath;

const HALF_LIFE_SECS: f64 = 259_200.0; // 3 days

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_secs()
}

#[derive(Clone, Debug)]
pub struct FrecencyEntry {
    pub score: f64,
    pub last_accessed: Option<u64>,
}

impl FrecencyEntry {
    pub fn new(score: f64, last_accessed: Option<u64>) -> Self {
        Self {
            score,
            last_accessed,
        }
    }

    /// Compute the current effective score by applying time decay from last_accessed to now.
    pub fn current_score(&self, now: u64) -> f64 {
        match self.last_accessed {
            Some(ts) => {
                let dt = now.saturating_sub(ts) as f64;
                self.score * (2.0_f64).powf(-dt / HALF_LIFE_SECS)
            }
            None => self.score,
        }
    }
}

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
    items: HashMap<PathBuf, FrecencyEntry>,
}

impl Cache {
    pub fn new() -> Self {
        let cache_filename = cache_filename();
        let items = if cache_filename.is_file() {
            let contents =
                std::fs::read_to_string(&cache_filename).expect("reading cache contents");
            contents
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    // Format: path\tscore\ttimestamp
                    // Note: paths containing literal tab characters are not supported.
                    let parts: Vec<&str> = line.split('\t').collect();
                    match parts.len() {
                        3 => {
                            let path = PathBuf::from(parts[0]);
                            let score = parts[1].parse::<f64>().unwrap_or(0.0);
                            let last_accessed = parts[2].parse::<u64>().ok();
                            (path, FrecencyEntry::new(score, last_accessed))
                        }
                        2 => {
                            let path = PathBuf::from(parts[0]);
                            let score = parts[1].parse::<f64>().unwrap_or(0.0);
                            (path, FrecencyEntry::new(score, None))
                        }
                        // 1 field = plain path (old format), >3 fields = malformed, treat as path
                        _ => (PathBuf::from(line), FrecencyEntry::new(0.0, None)),
                    }
                })
                .collect::<HashMap<PathBuf, FrecencyEntry>>()
        } else {
            HashMap::new()
        };

        Cache { items }
    }

    pub fn clear(&mut self) -> Result<(), io::Error> {
        self.items.clear();
        self.save_items_to(cache_filename());
        Ok(())
    }

    /// Implementation for prepopulating the cache with project names.
    /// Items are sorted by current frecency score descending (highest first),
    /// so skim places the most-used projects nearest the cursor.
    pub fn prepopulate_with(&self, tx: SkimItemSender) {
        let now = now_epoch();
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| {
            b.1.current_score(now)
                .partial_cmp(&a.1.current_score(now))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (p, _) in sorted {
            let item: Arc<dyn SkimItem> = Arc::new(SelectablePath { path: p.clone() });
            let _ = tx.send(item);
        }
    }

    /// Add an item to the cache if not already present, and return true if the cache was updated
    pub fn add_to_cache(&mut self, value: impl Into<PathBuf>) -> bool {
        let path = value.into();
        if self.items.contains_key(&path) {
            false
        } else {
            self.items.insert(path, FrecencyEntry::new(0.1, None));
            true
        }
    }

    /// Record a visit to a project, updating its frecency score.
    pub fn record_visit(&mut self, path: &Path) {
        let now = now_epoch();
        let entry = self
            .items
            .entry(path.to_path_buf())
            .or_insert_with(|| FrecencyEntry::new(0.0, None));
        let decayed = entry.current_score(now);
        entry.score = decayed + 1.0;
        entry.last_accessed = Some(now);
    }

    pub fn save(&self) -> Result<(), io::Error> {
        self.save_items_to(cache_filename());
        Ok(())
    }

    fn save_items_to(&self, output_path: impl AsRef<Path>) {
        let mut f = std::fs::File::create(output_path).expect("creating cache file");
        for (path, entry) in &self.items {
            match entry.last_accessed {
                Some(ts) => {
                    writeln!(f, "{}\t{:.6}\t{}", path.display(), entry.score, ts)
                        .expect("writing item to cache file");
                }
                None => {
                    writeln!(f, "{}\t{:.6}", path.display(), entry.score)
                        .expect("writing item to cache file");
                }
            }
        }
        f.flush().expect("flushing cache file");
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        self.save().expect("Failed to save cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const HALF_LIFE: f64 = 259_200.0; // 3 days in seconds

    // Helper: build a Cache from a file path (bypasses cache_filename())
    fn cache_from_file(path: &Path) -> Cache {
        let contents = std::fs::read_to_string(path).unwrap_or_default();
        let items = contents
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                match parts.len() {
                    3 => {
                        let path = PathBuf::from(parts[0]);
                        let score = parts[1].parse::<f64>().unwrap_or(0.0);
                        let last_accessed = parts[2].parse::<u64>().ok();
                        (path, FrecencyEntry::new(score, last_accessed))
                    }
                    2 => {
                        let path = PathBuf::from(parts[0]);
                        let score = parts[1].parse::<f64>().unwrap_or(0.0);
                        (path, FrecencyEntry::new(score, None))
                    }
                    _ => (PathBuf::from(line), FrecencyEntry::new(0.0, None)),
                }
            })
            .collect();
        Cache { items }
    }

    // Helper: create a temp file with given contents, returns path
    fn temp_cache_file(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("listprojects_test_{}", name));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        path
    }

    // ── Half-life decay formula tests ──

    #[test]
    fn first_visit_no_prior_score() {
        // score=0, no last_accessed → decayed is 0.0, new score = 0.0 + 1.0 = 1.0
        let entry = FrecencyEntry::new(0.0, None);
        let now = 1_710_700_800;
        let decayed = entry.current_score(now);
        let new_score = decayed + 1.0;
        assert!((new_score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn revisit_after_one_hour() {
        // score=4.0, Δt=3600s → 4.0 × 2^(-3600/259200) + 1.0 ≈ 4.962
        let ts = 1_710_700_800;
        let now = ts + 3600;
        let entry = FrecencyEntry::new(4.0, Some(ts));
        let decayed = entry.current_score(now);
        let new_score = decayed + 1.0;
        let expected = 4.0 * (2.0_f64).powf(-3600.0 / HALF_LIFE) + 1.0;
        assert!(
            (new_score - expected).abs() < 0.01,
            "expected ~{expected}, got {new_score}"
        );
        assert!(
            (new_score - 4.96).abs() < 0.05,
            "expected ~4.96, got {new_score}"
        );
    }

    #[test]
    fn revisit_after_30_days() {
        // score=4.0, Δt=2592000s → 4.0 × 2^(-2592000/259200) + 1.0 ≈ 1.004
        let ts = 1_710_700_800;
        let now = ts + 2_592_000;
        let entry = FrecencyEntry::new(4.0, Some(ts));
        let decayed = entry.current_score(now);
        let new_score = decayed + 1.0;
        let expected = 4.0 * (2.0_f64).powf(-2_592_000.0 / HALF_LIFE) + 1.0;
        assert!(
            (new_score - expected).abs() < 0.001,
            "expected ~{expected}, got {new_score}"
        );
        assert!(
            (new_score - 1.004).abs() < 0.01,
            "expected ~1.004, got {new_score}"
        );
    }

    #[test]
    fn current_score_no_last_accessed_returns_raw() {
        let entry = FrecencyEntry::new(3.5, None);
        assert!((entry.current_score(9_999_999_999) - 3.5).abs() < 1e-9);
    }

    #[test]
    fn current_score_dt_zero_returns_raw() {
        let ts = 1_710_700_800;
        let entry = FrecencyEntry::new(3.5, Some(ts));
        // 2^0 = 1, so score * 1 = score
        assert!((entry.current_score(ts) - 3.5).abs() < 1e-9);
    }

    // ── Cache parsing tests ──

    #[test]
    fn parse_plain_path_no_tabs() {
        let path = temp_cache_file("plain_path", "/home/user/dev/project\n");
        let cache = cache_from_file(&path);
        let entry = cache
            .items
            .get(&PathBuf::from("/home/user/dev/project"))
            .unwrap();
        assert!((entry.score - 0.0).abs() < 1e-9);
        assert!(entry.last_accessed.is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn parse_tab_separated_line() {
        let path = temp_cache_file("tab_sep", "/home/user/dev/project\t4.200000\t1710700800\n");
        let cache = cache_from_file(&path);
        let entry = cache
            .items
            .get(&PathBuf::from("/home/user/dev/project"))
            .unwrap();
        assert!((entry.score - 4.2).abs() < 1e-6);
        assert_eq!(entry.last_accessed, Some(1_710_700_800));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn parse_score_no_timestamp() {
        let path = temp_cache_file("score_no_ts", "/home/user/dev/project\t2.500000\n");
        let cache = cache_from_file(&path);
        let entry = cache
            .items
            .get(&PathBuf::from("/home/user/dev/project"))
            .unwrap();
        assert!((entry.score - 2.5).abs() < 1e-6);
        assert!(entry.last_accessed.is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn parse_empty_lines_skipped() {
        let path = temp_cache_file("empty_lines", "/home/a\n\n/home/b\n\n");
        let cache = cache_from_file(&path);
        assert_eq!(cache.items.len(), 2);
        std::fs::remove_file(&path).ok();
    }

    // ── Cache serialization tests ──

    #[test]
    fn serialize_entry_with_timestamp() {
        let mut items = HashMap::new();
        items.insert(
            PathBuf::from("/home/user/dev/proj"),
            FrecencyEntry::new(4.2, Some(1_710_700_800)),
        );
        let cache = Cache { items };
        let out = std::env::temp_dir().join("listprojects_test_ser_ts");
        cache.save_items_to(&out);
        let contents = std::fs::read_to_string(&out).unwrap();
        assert!(contents.contains("/home/user/dev/proj\t4.200000\t1710700800"));
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn serialize_entry_without_timestamp() {
        let mut items = HashMap::new();
        items.insert(
            PathBuf::from("/home/user/dev/proj"),
            FrecencyEntry::new(0.1, None),
        );
        let cache = Cache { items };
        let out = std::env::temp_dir().join("listprojects_test_ser_no_ts");
        cache.save_items_to(&out);
        let contents = std::fs::read_to_string(&out).unwrap();
        let line = contents.trim();
        assert_eq!(line, "/home/user/dev/proj\t0.100000");
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn round_trip_preserves_data() {
        let mut items = HashMap::new();
        items.insert(
            PathBuf::from("/home/user/dev/alpha"),
            FrecencyEntry::new(3.14, Some(1_710_700_800)),
        );
        items.insert(
            PathBuf::from("/home/user/dev/beta"),
            FrecencyEntry::new(0.5, None),
        );
        let cache = Cache { items };
        let out = std::env::temp_dir().join("listprojects_test_roundtrip");
        cache.save_items_to(&out);

        let restored = cache_from_file(&out);
        assert_eq!(restored.items.len(), 2);

        let alpha = restored
            .items
            .get(&PathBuf::from("/home/user/dev/alpha"))
            .unwrap();
        assert!((alpha.score - 3.14).abs() < 0.001);
        assert_eq!(alpha.last_accessed, Some(1_710_700_800));

        let beta = restored
            .items
            .get(&PathBuf::from("/home/user/dev/beta"))
            .unwrap();
        assert!((beta.score - 0.5).abs() < 0.001);
        assert!(beta.last_accessed.is_none());

        std::fs::remove_file(&out).ok();
    }

    // ── add_to_cache tests ──

    #[test]
    fn add_to_cache_new_item_gets_initial_score() {
        let mut cache = Cache {
            items: HashMap::new(),
        };
        assert!(cache.add_to_cache("/home/user/dev/new_project"));
        let entry = cache
            .items
            .get(&PathBuf::from("/home/user/dev/new_project"))
            .unwrap();
        assert!((entry.score - 0.1).abs() < 1e-9);
        assert!(entry.last_accessed.is_none());
    }

    #[test]
    fn add_to_cache_existing_item_returns_false() {
        let mut cache = Cache {
            items: HashMap::new(),
        };
        cache.add_to_cache("/home/user/dev/existing");
        assert!(!cache.add_to_cache("/home/user/dev/existing"));
    }

    // ── record_visit tests ──

    #[test]
    fn record_visit_new_path_creates_entry_with_score_1() {
        let mut cache = Cache {
            items: HashMap::new(),
        };
        cache.record_visit(Path::new("/home/user/dev/fresh"));
        let entry = cache
            .items
            .get(&PathBuf::from("/home/user/dev/fresh"))
            .unwrap();
        // or_insert gives score=0.0, current_score(now)=0.0, new = 0.0 + 1.0 = 1.0
        assert!((entry.score - 1.0).abs() < 1e-9);
        assert!(entry.last_accessed.is_some());
    }

    #[test]
    fn record_visit_existing_path_applies_decay_plus_one() {
        let mut items = HashMap::new();
        let ts = now_epoch() - 3600; // 1 hour ago
        items.insert(
            PathBuf::from("/home/user/dev/proj"),
            FrecencyEntry::new(4.0, Some(ts)),
        );
        let mut cache = Cache { items };
        cache.record_visit(Path::new("/home/user/dev/proj"));
        let entry = cache
            .items
            .get(&PathBuf::from("/home/user/dev/proj"))
            .unwrap();
        // decayed = 4.0 * 2^(-3600/259200) ≈ 3.962, new = decayed + 1.0 ≈ 4.962
        let expected_decayed = 4.0 * (2.0_f64).powf(-3600.0 / HALF_LIFE);
        assert!(
            (entry.score - (expected_decayed + 1.0)).abs() < 0.01,
            "expected ~{}, got {}",
            expected_decayed + 1.0,
            entry.score
        );
        assert!(entry.last_accessed.is_some());
    }
}
