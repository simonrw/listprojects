## Context

Project activation is currently embedded in `src/main.rs` as a `Tmux` type. Both interactive selection and `--path` eventually construct that type, but tmux-specific branching, subprocess execution, and process replacement are coupled together. Herdr exposes workspace operations through JSON-returning CLI commands and identifies managed panes with `HERDR_ENV=1`; its activation lifecycle therefore differs from tmux's session lifecycle.

The installed Herdr CLI provides the required stable surfaces: `herdr workspace list`, `herdr workspace focus <workspace_id>`, and `herdr workspace create --cwd <path> --label <name> --focus`.

## Goals / Non-Goals

**Goals:**
- Keep one activation entry point shared by interactive and `--path` flows.
- Make Herdr-versus-tmux selection explicit and testable.
- Reuse Herdr workspaces by the standardized final-component label.
- Preserve subprocess errors with enough command context to diagnose failures.

**Non-Goals:**
- Adding a user-selectable multiplexer configuration flag.
- Discovering or controlling Herdr from outside a Herdr-managed pane.
- Distinguishing same-named projects by full path; the standardized label intentionally defines their shared identity.
- Managing tabs or panes inside a Herdr workspace.

## Decisions

### 1. Select an activation backend before activating

Introduce a small activation boundary that selects Herdr when `HERDR_ENV` is exactly `1`, otherwise tmux. Selection occurs once per chosen project before any backend command runs, so a nested tmux value cannot cause a tmux probe or side effect after Herdr has been selected.

**Alternatives considered:**
- Check whether the `herdr` binary exists: installation does not mean the caller belongs to a Herdr session, and Herdr's own control guidance requires `HERDR_ENV=1`.
- Try Herdr and fall back on command failure: this could hide real Herdr failures and unexpectedly activate tmux.

### 2. Use the exact final path component as shared activation identity

Replace `compute_session_name`'s `parent/project` result and dot rewriting with an activation-name helper that returns the canonical path's final component. The value is passed as an argument rather than shell-interpolated, preserving spaces and other non-shell characters. Missing final components produce a structured error rather than a panic.

This deliberately means two paths with the same basename collide: Herdr workspace reuse and tmux session reuse both follow the requested name identity.

**Alternatives considered:**
- Keep `parent/project`: conflicts with the requested workspace labels.
- Store full paths in Herdr metadata: workspace list does not expose a cwd for matching, and path-aware disambiguation would contradict basename-only identity.

### 3. Resolve Herdr workspaces from structured CLI output

Run `herdr workspace list`, require a successful exit status, and parse its JSON response. Inspect `result.workspaces` for exact label equality. If several match, choose the lowest numeric `number` to make legacy duplicate handling deterministic, then invoke `herdr workspace focus` with its opaque `workspace_id`.

If none match, invoke:

`herdr workspace create --cwd <canonical-path> --label <activation-name> --focus`

Add `serde_json` as a direct dependency and parse only the response fields needed for matching. Treat malformed JSON, missing required fields, and unsuccessful commands as activation errors; do not fall back to tmux after Herdr selection.

**Alternatives considered:**
- Parse JSON manually: brittle around escaping and future response additions.
- Match the first array entry: current ordering is observable but choosing the lowest workspace number documents deterministic behavior.
- Always create a workspace: violates the requested reuse behavior.

### 4. Keep tmux as a backend with standardized naming

Retain tmux's existing inside-session behavior: probe the named session, create it at the project path if missing, and switch the current client. Outside tmux, probe before creation so an existing standardized session can be attached rather than failing an unconditional `new-session`, then replace the process with `attach-session` as today.

Refactor activation methods to return `eyre::Result` consistently where possible, preserving command-status and spawn context instead of using `expect`.

### 5. Isolate environment and command execution for tests

Keep backend selection as a pure function over captured environment indicators. Put subprocess invocation behind a narrow command-runner boundary so tests can supply JSON/status fixtures and assert command order and arguments without requiring live Herdr or tmux servers. Production continues to use `std::process::Command` and Unix `exec` for the final tmux switch/attach operations.

**Alternatives considered:**
- Environment mutation and real subprocesses in unit tests: global environment is race-prone, and live multiplexer state would make tests destructive and non-deterministic.
- End-to-end tests only: insufficient coverage for precedence and failure paths.

## Risks / Trade-offs

- **[Basename collisions activate a workspace/session belonging to another path]** → This is an intentional consequence of the requested basename-only identity; document it in tests and user-facing change notes.
- **[Herdr JSON response shape changes]** → Parse only required fields, ignore unknown fields, and report a contextual error instead of silently creating duplicates.
- **[A stale duplicate Herdr label exists]** → Select the lowest workspace number deterministically and avoid creating an additional duplicate.
- **[Herdr command fails while tmux is also detected]** → Return the Herdr error without tmux fallback, preserving the precedence guarantee and preventing surprising side effects.
- **[Some basenames are awkward tmux target names]** → Pass names as direct arguments and add tests for dots and spaces; report tmux rejection rather than introducing backend-specific renaming that would break shared identity.

## Migration Plan

1. Add JSON parsing support and the backend-selection/command-runner boundaries.
2. Implement and test Herdr workspace lookup, focus, and creation.
3. Route both activation entry points through the shared selector and update tmux naming/reuse tests.
4. Run formatting, unit tests, clippy, and strict OpenSpec validation.

Rollback consists of reverting the activation refactor and removing the new JSON dependency. Existing Herdr workspaces created by the feature are ordinary user workspaces and are not deleted during rollback.
