## 1. Activation Boundary and Naming

- [x] 1.1 Add unit tests for exact final-path-component activation names, including nested paths, dots, spaces, same-basename collisions, and paths without a usable final component
- [x] 1.2 Replace the tmux-specific session-name helper with the shared, fallible activation-name helper
- [x] 1.3 Add a pure backend-selection function and tests proving `HERDR_ENV=1` selects Herdr ahead of `TMUX`, while all other Herdr values select tmux
- [x] 1.4 Introduce the command-runner/activation boundary needed to test subprocess behavior without live multiplexer state

## 2. Herdr Workspace Activation

- [x] 2.1 Add `serde_json` as a direct dependency for Herdr CLI response parsing
- [x] 2.2 Add fixture-based tests for exact-label workspace matching, no match, and deterministic lowest-workspace-number selection among duplicate labels
- [x] 2.3 Implement `herdr workspace list` execution, status validation, and contextual parsing of the required workspace fields
- [x] 2.4 Add command-runner tests proving a match invokes only `herdr workspace focus <workspace_id>`
- [x] 2.5 Add command-runner tests proving no match invokes `herdr workspace create --cwd <path> --label <name> --focus`
- [x] 2.6 Implement Herdr focus/create activation and propagate command, status, and malformed-response failures without falling back to tmux

## 3. Tmux Fallback

- [x] 3.1 Update tmux tests and implementation to use the shared basename-only activation name
- [x] 3.2 Test and implement tmux create-or-reuse behavior for callers both inside and outside tmux
- [x] 3.3 Replace activation-time `expect` calls with contextual results while retaining process replacement for tmux switch and attach

## 4. Entry-Point Integration

- [x] 4.1 Route interactive project selection through the shared backend selector and activation boundary
- [x] 4.2 Route `--path` activation through the same selector after canonicalization and cache visit recording
- [x] 4.3 Update CLI help text that currently describes `--path` as tmux-only
- [x] 4.4 Add integration-level command-runner tests confirming Herdr precedence causes no tmux commands when both environment indicators are present

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo test`, and `cargo clippy -- -D warnings`, resolving all failures
- [x] 5.2 Run strict OpenSpec validation for `add-herdr-workspace-support` and confirm all project-activation scenarios are represented by tests
