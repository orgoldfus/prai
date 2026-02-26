mod agent;
mod app;
mod config;
mod git;
mod github;
mod ui;

use anyhow::{Result, bail};
use clap::Parser;

use crate::config::Config;
use crate::github::client::GitHubClient;

/// 🙏 PRAI — AI-Powered Code Review Assistant
#[derive(Parser)]
#[command(name = "prai", version, about)]
struct Cli {
    /// PR number to review directly (skips PR selection).
    pr_number: Option<u64>,

    /// Open the config file in your default editor.
    #[arg(long)]
    config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle `--config` flag.
    if cli.config {
        let path = Config::path()?;
        // Ensure a default config exists.
        let _ = Config::load()?;
        println!("Config file: {}", path.display());
        open::that(&path)?;
        return Ok(());
    }

    // ── Pre-flight checks ─────────────────────────────────────────────

    if !git::is_git_repo() {
        bail!(
            "🙏 PRAI must be run inside a git repository.\n   \
             Please navigate to a repo and try again."
        );
    }

    GitHubClient::check_auth().await?;

    // ── Load config & launch ──────────────────────────────────────────

    let config = Config::load()?;
    let app = app::App::new(config, cli.pr_number).await?;

    let terminal = ratatui::init();
    let result = app.run(terminal).await;
    ratatui::restore();

    result
}
