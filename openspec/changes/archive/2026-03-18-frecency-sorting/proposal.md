## Why

Projects are currently displayed in arbitrary order (hash-set iteration order for cached items, parallel walk order for new discoveries). This makes it hard to quickly select frequently-used projects without fuzzy searching every time. Sorting by frecency (frequency + recency) puts the most relevant projects nearest the cursor.

## What Changes

- Extend `cache.txt` format to store a frecency score and last-accessed timestamp per path (tab-separated: `path\tscore\ttimestamp`)
- Implement half-life decay scoring: `score = old_score × 2^(-Δt / half_life) + 1.0` on each project selection
- Sort cached items by score (descending) before sending to skim, so highest-scored items appear nearest the cursor
- Assign a small initial score (e.g. 0.1) to newly discovered projects so they appear below used projects but aren't invisible
- Update the score on project selection (both interactive and `--path` modes) and persist to disk
- Backwards-compatible parsing: lines without score/timestamp data are treated as score 0

## Capabilities

### New Capabilities
- `frecency-scoring`: Half-life decay algorithm for computing and updating project frecency scores
- `sorted-display`: Ordering of projects by frecency score when presented to the user

### Modified Capabilities

## Impact

- `disk_cache.rs`: Major changes — new data structure (score + timestamp per entry), extended file format parsing/writing, sorted output
- `main.rs`: Update selection handler to record a visit (update score) before saving cache; ensure `--path` mode also records visits
- Cache file format: Extended but backwards-compatible (old single-path lines still parse)
