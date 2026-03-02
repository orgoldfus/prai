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
- 🎨 **Beautiful TUI** — Catppuccin Mocha theme, diff syntax highlighting
- ⌨️ **Vim-style navigation** — j/k, Enter, Space, and more

## Prerequisites

- [GitHub CLI (`gh`)](https://cli.github.com) — installed and authenticated
- [Cursor CLI](https://cursor.com/cli) — for the AI agent integration
- Git repository with a GitHub remote

## Installation

### From source

```bash
cargo install --path .
```

### From crates.io (coming soon)

```bash
cargo install prai
```

## Usage

```bash
# Auto-detect PR for current branch
prai

# Review a specific PR
prai 42

# Open config file
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
| `a` | Send to AI agent |
| `l` | Toggle agent output panel |
| `v` | Toggle output view (UI/raw) |
| `[` / `]` | Switch agent job in panel |
| `m` | Choose model |
| `o` | Open in browser |
| `t` | 👍 React |
| `r` | Reply (coming soon) |
| `Enter` | View detail |
| `q` | Back |
| `Ctrl+C` | Quit |

## Configuration

Config file: `~/.config/prai/config.toml`

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

- **`GitProvider` trait** — currently GitHub via `gh` CLI, easily extendable to GitLab/Bitbucket
- **`AgentProvider` trait** — currently Cursor CLI, easily extendable to Claude Code, Aider, etc.
- **ratatui** — fast, lightweight TUI framework
- **GraphQL** — uses GitHub's GraphQL API to get review thread resolution status

## License

MIT
