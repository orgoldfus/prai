# 🙏 PRAI

**AI-Powered Code Review Assistant**

PRAI is a terminal UI tool that lets you browse your PR review comments and send them to an AI agent to fix — all without leaving the terminal.

```
        🙏

 ██████╗ ██████╗  █████╗ ██╗
 ██╔══██╗██╔══██╗██╔══██╗██║
 ██████╔╝██████╔╝███████║██║
 ██╔═══╝ ██╔══██╗██╔══██║██║
 ██║     ██║  ██║██║  ██║██║
 ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝

 AI-Powered Code Review Assistant
```

## Features

- 🔍 **Auto-detect PRs** — automatically finds the PR for your current branch
- 💬 **Browse review comments** — view unresolved inline comments with code context
- ✅ **Multi-select** — select multiple comments to fix in one batch
- 🤖 **Send to AI agent** — dispatch comments to Cursor CLI (more agents coming)
- 💬 **Reply to threads** — post replies directly from the TUI
- 🎨 **Beautiful TUI** — Catppuccin Mocha theme, diff syntax highlighting
- ⌨️ **Vim-style navigation** — j/k, Enter, Space, and more

## Prerequisites

- [GitHub CLI (`gh`)](https://cli.github.com) — installed and authenticated
- [Cursor CLI](https://cursor.com/cli) — for the AI agent integration
- Git repository with a GitHub remote

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Auto-detect PR for current branch
prai

# Review a specific PR
prai 42

# Open config file in your editor
prai --config
```

## Keybindings

### PR Selection

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate |
| `Enter` | Select PR |
| `q` | Quit |

### Comment List

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate |
| `Space` | Toggle select |
| `Ctrl+a` | Select all |
| `Ctrl+d` | Deselect all |
| `a` | Send to AI agent (with optional instructions) |
| `l` | Toggle agent output panel |
| `v` | Toggle output view (UI/raw) |
| `[` / `]` | Switch agent job in panel |
| `m` | Choose model |
| `o` | Open in browser |
| `t` | 👍 React |
| `r` | Reply to thread |
| `Enter` | View detail |
| `q` | Back to PR list |
| `Ctrl+C` | Quit |

### Comment Detail

| Key | Action |
|-----|--------|
| `a` | Send this comment to agent |
| `r` | Reply to thread |
| `o` | Open in browser |
| `t` | 👍 React |
| `q` | Back to comment list |

### Reply / Additional Instructions Popup

| Key | Action |
|-----|--------|
| `Ctrl+s` | Submit |
| `Esc` | Cancel |

## Configuration

Config file: `~/.config/prai/config.toml` (auto-created on first run)

```toml
[agent]
provider = "cursor"           # AI agent to use
default_model = "claude-4-sonnet"  # Default model

[ui]
theme = "catppuccin-mocha"    # Color theme
splash_duration_ms = 1500     # Splash screen duration
```

## Architecture

PRAI is built with extensibility in mind:

- **`GitProvider` trait** (`src/github/provider.rs`) — abstracts git hosting. Currently GitHub via `gh` CLI; extendable to GitLab, Bitbucket, etc.
- **`AgentProvider` trait** (`src/agent/provider.rs`) — abstracts AI coding agents. Currently Cursor CLI; extendable to Claude Code, Aider, etc.
- **ratatui** — fast, lightweight TUI framework with Catppuccin Mocha theme
- **GraphQL** — uses GitHub's GraphQL API for review thread resolution status

### Module overview

```
src/
├── main.rs              CLI entry point, preflight checks
├── config.rs            TOML config loading/saving
├── git.rs               Git helpers (branch, remote URL parsing)
├── app/
│   ├── mod.rs           App struct, main loop, rendering dispatch
│   ├── keys.rs          Keyboard event handlers for each screen
│   └── actions.rs       Agent dispatch, reply submission, transitions
├── agent/
│   ├── provider.rs      AgentProvider trait
│   ├── cursor.rs        Cursor CLI implementation
│   ├── stream.rs        Agent output stream parsing
│   └── mod.rs           Prompt building
├── github/
│   ├── provider.rs      GitProvider trait
│   ├── client.rs        GitHub CLI / GraphQL implementation
│   └── types.rs         Data types (PullRequest, ReviewComment, etc.)
└── ui/
    ├── mod.rs            Model selector popup, shared utilities
    ├── theme.rs          Catppuccin Mocha palette and semantic styles
    ├── splash.rs         Splash screen
    ├── pr_list.rs        PR selection screen
    ├── comment_list.rs   Comment list screen and agent panel
    ├── comment_detail.rs Full-screen comment detail view
    ├── reply.rs          Reply popup
    ├── additional_instructions.rs  Additional instructions popup
    ├── agent_timeline.rs Agent output timeline rendering
    ├── text_buffer.rs    Multi-line text input buffer
    └── status_bar.rs     Bottom status bar with key hints
```

## Development

```bash
cargo check              # Type-check without building
cargo build              # Debug build
cargo build --release    # Release build (LTO + stripped)
cargo test               # Run tests
cargo test parse_ssh_url # Run a specific test
cargo clippy             # Lint
cargo run                # Run directly
cargo run -- 42          # Specific PR number
cargo run -- --config    # Open config file
```

### For AI agents

See [`AGENTS.md`](./AGENTS.md) for detailed guidance on working with this codebase.

## License

MIT
