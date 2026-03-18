## Context

`listprojects` discovers git repositories and presents them via skim for interactive selection. Currently, items are stored in a `HashSet<PathBuf>` and arrive at skim in arbitrary order. The user wants frequently and recently used projects to appear nearest the cursor (bottom of the list in skim's default layout).

## Goals / Non-Goals

**Goals:**
- Sort projects by frecency so the most relevant ones are nearest the cursor
- Persist frecency data in the existing cache file with backwards-compatible format
- Update scores on every project selection (interactive and `--path` modes)

**Non-Goals:**
- Configurable half-life value (hardcode a sensible default)
- Frecency-aware fuzzy matching (skim still does its own ranking when the user types a query)
- Migrating to a database or separate storage format

## Decisions

### 1. Algorithm: Half-life exponential decay

**Choice:** `score = old_score × 2^(-Δt / half_life) + 1.0`

**Alternatives considered:**
- Zoxide-style discrete time buckets — simpler but creates score discontinuities at bucket boundaries
- Pure frequency counting — no recency component, stale projects stay at top forever

**Rationale:** Half-life decay is smooth, stores only two values per item (score + timestamp), and naturally ages out unused projects. The formula compresses the full visit history into a single running score.

### 2. Half-life value: 3 days (259200 seconds)

A visit from 3 days ago contributes half as much as a visit right now. This is responsive enough for a project switcher where daily usage patterns matter.

### 3. Storage: Extend cache.txt with tab-separated fields

**Format:** `path\tscore\tlast_accessed_epoch`

Example:
```
/home/simon/dev/listprojects	4.200000	1710700800
/home/simon/dev/other-project	1.500000	1710500000
/home/simon/work/new-thing
```

**Alternatives considered:**
- Separate JSON file — cleaner separation but adds file management complexity
- SQLite — far too heavy for this use case

**Rationale:** Tab-separated extends naturally. Lines without score data (no tabs, or from old format) parse as path-only with score 0. No new files or dependencies needed.

### 4. Data structure: Replace HashSet with HashMap

Change `HashSet<PathBuf>` to `HashMap<PathBuf, FrecencyEntry>` where `FrecencyEntry` holds `score: f64` and `last_accessed: Option<u64>` (epoch seconds).

### 5. Sort order for skim

Send items to skim sorted by score **descending** — highest scores first. In skim's default bottom-up layout, first-sent items appear at the bottom (nearest cursor). This means most-used projects are immediately accessible.

### 6. Initial score for new projects

Newly discovered projects get score `0.1`. This puts them below any project the user has actually selected (minimum score after one visit ≈ 1.0) but above zero, keeping them visible.

### 7. When to update scores

Score is updated when a project is **selected** — either via interactive skim selection or via `--path` flag. The cache must be loaded, the score updated, and the cache saved in both paths.

## Risks / Trade-offs

- **Floating-point drift**: After many updates, f64 scores could accumulate precision errors → Negligible for this use case; scores naturally stay small due to decay
- **Cache file size**: Adding two fields per line increases file size → Minimal impact, typically <100 projects
- **Background-discovered items arrive unsorted**: New items from the walker thread are sent to skim as discovered and can't be pre-sorted → Acceptable since these are new/uncached projects with score 0.1 and will appear at the top (far from cursor), which is correct behavior
