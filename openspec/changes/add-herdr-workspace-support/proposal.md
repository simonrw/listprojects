## Why

`listprojects` currently assumes tmux for project activation, so selecting a project from a Herdr-managed pane cannot switch to or create the corresponding Herdr workspace. Project activation should follow the active terminal multiplexer while retaining tmux behavior outside Herdr.

## What Changes

- Detect Herdr-managed execution through Herdr's injected environment and choose Herdr activation before considering tmux, including when both multiplexers are detected.
- In Herdr, reuse and focus a workspace whose label matches the selected project's final path component; otherwise create and focus a workspace rooted at the selected project.
- Outside Herdr, preserve tmux activation as the fallback.
- **BREAKING** Standardize project activation names to the selected path's final component only, replacing tmux's current `parent/project` session naming. Projects with the same final component therefore share an activation name.
- Apply the same multiplexer selection and naming behavior to both interactive selection and `--path` activation.

## Capabilities

### New Capabilities
- `project-activation`: Selection of a terminal multiplexer and activation of a named project workspace or session.

### Modified Capabilities

None.

## Impact

- `src/main.rs`: Replace unconditional `Tmux` construction with multiplexer selection, add Herdr workspace lookup/create/focus behavior, and simplify activation-name computation.
- External commands: add calls to `herdr workspace list`, `herdr workspace focus`, and `herdr workspace create`; retain existing tmux commands as fallback.
- Environment contract: use `HERDR_ENV=1` as the authoritative Herdr-session signal and continue using `TMUX` for tmux detection.
- Tests: cover activation naming, detection precedence, existing-workspace reuse, workspace creation, and tmux fallback behavior.
