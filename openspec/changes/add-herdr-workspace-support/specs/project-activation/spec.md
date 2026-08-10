## Purpose

Defines how a selected project is named and activated in the user's current terminal-multiplexer environment.

## ADDED Requirements

### Requirement: Project activation name
The system SHALL derive the activation name from only the final path component of the selected project's canonical path. The same activation name SHALL be used for Herdr workspace labels and tmux session names.

#### Scenario: Nested project path
- **WHEN** the selected canonical path is `/Users/simon/work/localstack/localstack-pro`
- **THEN** the activation name is `localstack-pro`

#### Scenario: Projects share a final component
- **WHEN** two selected project paths have the same final path component
- **THEN** the system treats them as the same activation name

### Requirement: Herdr detection takes precedence
The system SHALL select Herdr activation when the process is running in a Herdr-managed environment. Herdr activation SHALL take precedence when both Herdr and tmux session indicators are present.

#### Scenario: Herdr environment only
- **WHEN** `HERDR_ENV` equals `1`
- **THEN** the system activates the selected project through Herdr

#### Scenario: Nested Herdr and tmux environment
- **WHEN** `HERDR_ENV` equals `1` and `TMUX` is also present
- **THEN** the system activates the selected project through Herdr and does not invoke tmux

#### Scenario: Herdr indicator is absent
- **WHEN** `HERDR_ENV` does not equal `1`
- **THEN** the system uses tmux activation

### Requirement: Existing Herdr workspace reuse
When Herdr activation is selected, the system SHALL reuse an existing workspace whose label exactly matches the activation name instead of creating another workspace. The reused workspace SHALL receive focus.

#### Scenario: Matching workspace exists
- **WHEN** a Herdr workspace label exactly matches the selected project's activation name
- **THEN** the system focuses that workspace and does not create a workspace

#### Scenario: Only non-matching workspaces exist
- **WHEN** Herdr workspaces exist but none has a label exactly matching the activation name
- **THEN** the system does not focus any of those workspaces as the project workspace

#### Scenario: Multiple matching workspaces exist
- **WHEN** more than one Herdr workspace has the matching label
- **THEN** the system focuses the matching workspace with the lowest workspace number and does not create another workspace

### Requirement: New Herdr workspace creation
When no existing Herdr workspace has the activation name, the system SHALL create and focus a workspace labeled with the activation name and rooted at the selected project's canonical path.

#### Scenario: No matching workspace exists
- **WHEN** Herdr activation is selected and no workspace label exactly matches the activation name
- **THEN** the system creates a focused workspace with that label and the selected canonical path as its working directory

### Requirement: Tmux fallback activation
When Herdr activation is not selected, the system SHALL retain tmux's create-or-reuse behavior using the standardized activation name.

#### Scenario: Existing tmux session from inside tmux
- **WHEN** Herdr is not selected, the process is inside tmux, and a session with the activation name exists
- **THEN** the system switches the current tmux client to that session

#### Scenario: Missing tmux session from inside tmux
- **WHEN** Herdr is not selected, the process is inside tmux, and no session with the activation name exists
- **THEN** the system creates that session at the selected project path and switches the current client to it

#### Scenario: Activation from outside a multiplexer
- **WHEN** Herdr is not selected and the process is outside tmux
- **THEN** the system creates or reuses the named tmux session at the selected project path and attaches to it

### Requirement: Consistent activation entry points
The system SHALL apply the same naming, multiplexer selection, reuse, and creation behavior after interactive project selection and after project selection through `--path`.

#### Scenario: Interactive selection
- **WHEN** the user selects a project in the interactive finder
- **THEN** the system activates it according to the project activation requirements

#### Scenario: Path shortcut
- **WHEN** the user supplies a valid project through `--path`
- **THEN** the system activates it according to the same project activation requirements
