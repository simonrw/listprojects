### Requirement: Cached items sorted by frecency score
The system SHALL send cached items to the skim fuzzy finder sorted by frecency score in descending order (highest score first).

#### Scenario: Prepopulating skim with sorted items
- **WHEN** cached items are sent to skim during startup
- **THEN** items SHALL be sent in descending order of their current frecency score (computed at query time using the half-life decay formula against the current time)

#### Scenario: Most-used project nearest cursor
- **WHEN** the skim interface is displayed with default layout
- **THEN** the highest-scored project SHALL appear at the bottom of the list (nearest the cursor), because skim places first-received items at the bottom

### Requirement: Background-discovered items use initial score
The system SHALL send newly discovered items (from the background directory walk) to skim as they are found, without re-sorting existing items.

#### Scenario: New project found during background scan
- **WHEN** a new project is discovered by the background walker
- **THEN** it SHALL be sent to skim immediately with its initial score of 0.1, appearing above (further from cursor than) previously scored items
