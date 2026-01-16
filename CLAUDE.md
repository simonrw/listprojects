# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`listprojects` is a Rust CLI tool that lists Git repositories in designated root directories (default: `~/dev` and `~/work`) and launches a tmux session for the selected project. It uses fuzzy finding (skim) for interactive selection and maintains a disk cache for faster startup.

## Common Commands

### Building and Running
```bash
cargo build               # Build the project
cargo run                 # Run with default roots (~/dev, ~/work)
cargo run -- ~/projects   # Run with custom root directory
cargo run -- --clear      # Clear cache before running
```

### Testing
```bash
cargo test                                  # Run all tests
cargo test test_compute_session_name        # Run a specific test
```

### Linting
```bash
cargo clippy              # Run linter
cargo fmt                 # Format code
cargo check               # Check without building
```

## Architecture

### Core Workflow
1. **Cache Loading**: On startup, `Cache::new()` loads previously discovered projects from `~/.cache/listprojects/cache.txt`
2. **Pre-population**: Cached projects are immediately sent to the fuzzy finder via `prepopulate_with()`
3. **Background Scan**: A parallel directory walker scans root directories for `.git` folders
4. **Incremental Updates**: New projects found during scan are added to both cache and fuzzy finder in real-time
5. **Selection & Activation**: User selects a project, and a tmux session is created/switched

### Key Components

**main.rs**: Entry point containing:
- `Tmux` struct: Manages tmux session lifecycle (create/attach/switch)
  - Session names use format `{parent}/{directory}` (e.g., `dev/myproject`)
  - Handles both in-session (switch-client) and out-of-session (attach) scenarios
- Directory walking with `ignore` crate's `WalkBuilder` for parallel traversal
- Skips: `.venv`, `node_modules`, `venv`, `__pycache__`, `.jj` directories
- Uses `CommandExt::exec()` for tmux commands (replaces process, doesn't return)

**disk_cache.rs**: Persistent caching system:
- `Cache` struct with `HashSet<PathBuf>` for O(1) lookups
- `add_to_cache()` returns `true` only if item is new (prevents duplicate sends to skim)
- Cache file: plain text, one path per line
- Auto-saves on `Drop` and explicit `save()` call after selection
- Thread-safe via `Arc<Mutex<Cache>>`

### Threading Model
- Main thread: Runs skim fuzzy finder (blocking UI)
- Background thread: Walks directories and sends items via unbounded channel
- Cache is shared between threads using `Arc<Mutex<Cache>>`

### Platform-Specific Notes
- Uses `std::os::unix::process::CommandExt` (Unix-only)
- Detects system color theme (dark/light) for skim UI
- Cache directory determined by `dirs::cache_dir()`
