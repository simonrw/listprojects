### Requirement: Half-life decay scoring
The system SHALL compute frecency scores using the formula `new_score = old_score × 2^(-Δt / half_life) + 1.0`, where `Δt` is the elapsed time since the last access in seconds and `half_life` is 259200 seconds (3 days).

#### Scenario: First visit to a project
- **WHEN** a project with no prior score (score 0, no last_accessed) is selected
- **THEN** the project's score SHALL be set to 1.0 and last_accessed SHALL be set to the current time

#### Scenario: Repeat visit to a recently used project
- **WHEN** a project with score 4.0 and last_accessed 1 hour ago is selected
- **THEN** the score SHALL be updated to approximately `4.0 × 2^(-3600/259200) + 1.0 ≈ 4.96` and last_accessed SHALL be updated to the current time

#### Scenario: Visit to a project not used in a long time
- **WHEN** a project with score 4.0 and last_accessed 30 days ago is selected
- **THEN** the score SHALL be updated to approximately `4.0 × 2^(-2592000/259200) + 1.0 ≈ 1.004` and last_accessed SHALL be updated to the current time

### Requirement: Score update on interactive selection
The system SHALL update the frecency score of a project when the user selects it via the interactive fuzzy finder.

#### Scenario: User selects project in skim
- **WHEN** the user selects a project from the skim interface
- **THEN** the system SHALL update that project's frecency score before saving the cache

### Requirement: Score update on direct path selection
The system SHALL update the frecency score of a project when the user selects it via the `--path` flag.

#### Scenario: User uses --path flag
- **WHEN** the user runs `listprojects --path ~/dev/myproject`
- **THEN** the system SHALL load the cache, update the frecency score for that path, and save the cache

### Requirement: Initial score for new projects
The system SHALL assign a score of 0.1 to newly discovered projects that have never been selected.

#### Scenario: New project discovered during directory walk
- **WHEN** a new git repository is found that is not in the cache
- **THEN** it SHALL be added to the cache with score 0.1 and no last_accessed timestamp

### Requirement: Persistent storage of frecency data
The system SHALL store frecency scores and last-accessed timestamps in the cache file using tab-separated format: `path\tscore\tlast_accessed_epoch`.

#### Scenario: Cache file with frecency data
- **WHEN** the cache is saved to disk
- **THEN** each entry SHALL be written as `path\tscore\tlast_accessed_epoch` (one per line)

#### Scenario: Backwards-compatible parsing
- **WHEN** the cache file contains lines with only a path (no tabs)
- **THEN** the system SHALL parse them as entries with score 0.0 and no last_accessed timestamp
