## 1. Data Model

- [x] 1.1 Add `FrecencyEntry` struct with `score: f64` and `last_accessed: Option<u64>` fields to `disk_cache.rs`
- [x] 1.2 Replace `HashSet<PathBuf>` with `HashMap<PathBuf, FrecencyEntry>` in the `Cache` struct
- [x] 1.3 Add a `record_visit(&mut self, path: &Path)` method that applies the half-life decay formula and updates the entry

## 2. Cache File Format

- [x] 2.1 Update `Cache::new()` to parse tab-separated `path\tscore\ttimestamp` lines, falling back to score 0.0 for plain path lines
- [x] 2.2 Update `save_items()` to write entries in `path\tscore\ttimestamp` format
- [x] 2.3 Set initial score to 0.1 for newly discovered projects in `add_to_cache()`

## 3. Sorted Display

- [x] 3.1 Update `prepopulate_with()` to sort items by current frecency score (recomputed at query time) in descending order before sending to skim
- [x] 3.2 Verify that in `--list` mode, output is also sorted by frecency score

## 4. Score Updates on Selection

- [x] 4.1 In the interactive selection path (`main.rs`), call `record_visit()` on the chosen project before saving the cache
- [x] 4.2 In the `--path` path (`main.rs`), load the cache, call `record_visit()` on the given path, and save the cache

## 5. Tests

- [x] 5.1 Test half-life decay formula: verify score computation for first visit, recent revisit, and long-ago revisit
- [x] 5.2 Test cache parsing: verify backwards-compatible parsing of plain path lines and tab-separated lines
- [x] 5.3 Test cache serialization: verify entries are written in the correct tab-separated format
