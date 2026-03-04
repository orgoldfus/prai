# AGENTS.md

PRAI is a Rust TUI tool for browsing PR review comments and dispatching them to an AI agent for fixing. Uses `gh` CLI for GitHub interaction and `cursor-agent` CLI for AI agent integration.

## Build & Test Commands

```bash
cargo build                # dev build
cargo build --release      # release (LTO, stripped)
cargo run                  # run directly
cargo run -- 42            # specific PR number
cargo run -- --config      # open config file
cargo install --path .     # install locally

cargo test                 # all tests
cargo test <test_name>     # single test, e.g. cargo test parse_ssh_url
cargo check                # typecheck without building
cargo clippy               # lint
```

## Architecture

### Core Flow

`main.rs` → CLI parsing (clap) → preflight checks (git repo? gh auth?) → load config → create `App` → run TUI event loop.

### Screen & Popup State Machine (`src/app/`)

The `app` module is split into `mod.rs` (state + main loop + rendering), `keys.rs` (key handlers), and `actions.rs` (agent dispatch, reply submission).

Screens (`Screen` enum):

```
Splash → (auto-detect PR for branch) → CommentList
                                      ↘ PrList → CommentList
CommentList ↔ CommentDetail
```

Overlay popups (`Popup` enum) render on top of the current screen:
- `ModelSelector` — fuzzy-filter model picker
- `Reply` — compose and submit a review thread reply
- `AdditionalInstructions` — optional extra instructions before agent dispatch

`App` owns the current `Screen`, `Popup`, `GitHubClient`, agent job queue, and cached model state. The main loop polls background model fetches, agent job streams, and keyboard events at 50 ms intervals.

### Agent Jobs & Streaming (`src/agent/`)

Agent work is tracked as `AgentJob` structs with background `JoinHandle`s. `stream.rs` defines `StreamChunk` (stdout/stderr/system) and `AgentStreamEvent` (thinking, assistant, tool start/update/end, error, done). The stream parser handles JSON fragments from `cursor-agent --output-format stream-json`.

The `AgentTimeline` (`src/ui/agent_timeline.rs`) consumes these events to build a structured view of agent activity (thinking, tool calls, errors, completion).

### Provider Traits

Two extension-point traits define the boundaries to external services:

- **`GitProvider`** (`src/github/provider.rs`) — list PRs, fetch review threads, add reactions, reply to threads. Implemented by `GitHubClient` which uses `gh` CLI + GraphQL for thread resolution status.
- **`AgentProvider`** (`src/agent/provider.rs`) — list models, execute prompt. Implemented by `CursorAgent` which shells out to `cursor-agent`.

### Model Caching (`src/agent/cursor.rs`)

Models are fetched in a background tokio task at startup. Three-level fallback: live CLI → disk cache (`~/.config/prai/models_cache.json`) → compile-time defaults.

### UI Layer (`src/ui/`)

Built with `ratatui` + `crossterm`. Each screen/popup has its own module with a `render()` function and state struct. Shared utilities:
- `theme.rs` — Catppuccin Mocha semantic styles (`theme::accent()`, `theme::diff_add()`, etc.). All styling goes through these.
- `status_bar.rs` — reusable key-hint bar rendered at the bottom of popups.
- `text_buffer.rs` — multiline text input with cursor navigation (used by Reply and AdditionalInstructions).

### Configuration (`src/config.rs`)

TOML config at `~/.config/prai/config.toml`. Auto-created with defaults on first run. Sections: `[agent]` (provider, default_model) and `[ui]` (theme, splash_duration_ms).

## Key Conventions

- External process calls (`gh`, `cursor-agent`) use `tokio::process::Command`. Git helpers in `src/git.rs` use `std::process::Command` (sync).
- Error handling uses `anyhow` with `.context()` for wrapping.
- `Screen` transitions use `std::mem::replace` to move owned state between variants.
- UI state structs own both data and ratatui `ListState` for selection tracking.

## Code Quality Rules

- Idiomatic Rust: proper ownership/borrowing, meaningful types, `anyhow` error handling.
- DRY — extract shared logic into functions, traits, or modules.
- KISS — prefer straightforward solutions over clever ones.
- No AI slop: no redundant comments restating obvious code, no boilerplate doc comments that just restate the function signature, no filler. Comments explain *why*, not *what*.
