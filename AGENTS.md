# AGENTS.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

PRAI is a Rust terminal UI (TUI) tool that lets you browse PR review comments and dispatch them to an AI agent for fixing. It uses `gh` CLI for GitHub interaction and `cursor-agent` CLI for AI agent integration.

## Build & Development Commands

```bash
# Build
cargo build

# Build release (with LTO, stripped)
cargo build --release

# Run directly
cargo run
cargo run -- 42          # specific PR number
cargo run -- --config    # open config file

# Install locally
cargo install --path .

# Run tests
cargo test

# Run a single test
cargo test <test_name>
# e.g. cargo test parse_ssh_url

# Check without building
cargo check

# Lint
cargo clippy
```

## Architecture

### Core Flow

`main.rs` → CLI parsing (clap) → preflight checks (git repo? gh auth?) → load config → create `App` → run TUI event loop.

### Screen State Machine (`app.rs`)

The app uses a `Screen` enum to manage navigation between views:

```
Splash → (auto-detect PR for branch) → CommentList
                                      ↘ PrList → CommentList
CommentList ↔ CommentDetail
```

`App` owns the current `Screen`, the `GitHubClient`, the `CursorAgent`, and cached model state. The main loop in `App::run()` is a standard ratatui render-then-poll-events loop with async key handlers.

### Provider Traits

Two extension-point traits define the boundaries between the core app and external services:

- **`GitProvider`** (`src/github/provider.rs`) — abstracts git hosting (list PRs, fetch review threads, add reactions, reply). Currently implemented by `GitHubClient` which shells out to the `gh` CLI and uses GitHub's GraphQL API for thread resolution status.
- **`AgentProvider`** (`src/agent/provider.rs`) — abstracts AI coding agents (list models, execute prompt). Currently implemented by `CursorAgent` which shells out to `cursor-agent`.

To add a new git host or agent, implement the corresponding trait.

### GitHub Client (`src/github/client.rs`)

Uses `gh` CLI for REST-style operations and raw GraphQL queries (via `gh api graphql`) for data that requires it (review thread `isResolved` status). Types in `src/github/types.rs` are deserialized from `gh` JSON output using `serde_json`.

### Model Caching (`src/agent/cursor.rs`)

Models are fetched in a background tokio task at startup. Three-level fallback: live CLI fetch → disk cache (`~/.config/prai/models_cache.json`) → compile-time defaults. This keeps the app startup instant.

### UI Layer (`src/ui/`)

Built with `ratatui` and `crossterm`. Each screen has its own module with a `render()` function and state struct. The Catppuccin Mocha theme is defined in `theme.rs` as semantic style functions (e.g., `theme::accent()`, `theme::diff_add()`). All UI styling should go through these functions rather than using raw colors.

### Configuration (`src/config.rs`)

TOML config at `~/.config/prai/config.toml`. Auto-created with defaults on first run. Sections: `[agent]` (provider, default_model) and `[ui]` (theme, splash_duration_ms).

## Key Conventions

- All external process calls (`gh`, `cursor-agent`, `git`) use `tokio::process::Command` for async execution (except `git` helpers in `src/git.rs` which use `std::process::Command` since they're sync).
- Error handling uses `anyhow` throughout with `.context()` for wrapping.
- The `Screen` enum uses `std::mem::replace` to move state between screens (e.g., moving `CommentListState` into/out of `CommentDetail`).
- UI state structs (e.g., `CommentListState`, `PrListState`) own both data and ratatui `ListState` for selection tracking.

## Code Quality Rules

- All code must adhere to Rust best practices (idiomatic error handling, proper use of ownership/borrowing, meaningful types, etc.).
- Follow DRY (Don't Repeat Yourself) — extract shared logic into functions, traits, or modules rather than duplicating it.
- Follow KISS (Keep It Simple, Stupid) — prefer straightforward, readable solutions over clever or over-engineered ones.
- Avoid AI slop: no redundant or obvious comments (e.g., `// return the value` before a return statement), no boilerplate doc comments that just restate the function signature, and no filler text. Comments should only exist when they explain *why*, not *what*.
