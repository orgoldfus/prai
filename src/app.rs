use std::env;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;

use crate::agent::cursor::CursorAgent;
use crate::agent::provider::AgentProvider;
use crate::agent::{self};
use crate::config::Config;
use crate::git;
use crate::github::client::GitHubClient;
use crate::github::provider::GitProvider;
use crate::github::types::ReviewComment;
use crate::ui::comment_list::{CommentEntry, CommentListState};
use crate::ui::pr_list::PrListState;
use crate::ui::{self, ModelSelectorState, splash};

// ── Screens ───────────────────────────────────────────────────────────────

/// Which screen the app is currently showing.
enum Screen {
    /// Splash screen shown on startup.
    Splash { shown_at: Instant },
    /// PR selection list.
    PrList(PrListState),
    /// Comment list for a selected PR.
    CommentList(CommentListState),
    /// Full detail view of a single comment.
    CommentDetail {
        entry: CommentEntry,
        parent: Box<CommentListState>,
    },
}

/// Optional overlay popup on top of the current screen.
enum Popup {
    None,
    ModelSelector(ModelSelectorState),
}

// ── App ───────────────────────────────────────────────────────────────────

pub struct App {
    config: Config,
    github: GitHubClient,
    agent: CursorAgent,
    screen: Screen,
    popup: Popup,
    /// The selected model (may be changed via the model selector).
    selected_model: String,
    /// Model list — starts with the instant fallback (disk cache / compile-time
    /// defaults) and is replaced by the live-fetched list once the background
    /// task completes.
    cached_models: Vec<String>,
    /// Shared slot written by the background model-fetch task.
    bg_models: Arc<Mutex<Option<Vec<String>>>>,
    /// Cached repo info.
    repo_owner: String,
    repo_name: String,
    /// Whether the app should quit.
    should_quit: bool,
}

impl App {
    /// Create the app with the given config and optionally a pre-selected PR number.
    pub async fn new(config: Config, pr_number: Option<u64>) -> Result<Self> {
        let github = GitHubClient;
        let agent = CursorAgent;

        let repo_info = git::repo_info().context("failed to read git remote")?;

        // Use the instant fallback list (disk cache → compile-time defaults)
        // so the app starts immediately, without waiting for the CLI.
        let fallback = CursorAgent::fallback_models();
        let selected_model = config.agent.default_model.clone();

        // Spawn the live model fetch in the background. The result will be
        // picked up on the next event-loop tick (or when the user presses `m`).
        let bg_models: Arc<Mutex<Option<Vec<String>>>> = Arc::new(Mutex::new(None));
        {
            let slot = Arc::clone(&bg_models);
            tokio::spawn(async move {
                if let Ok(models) = CursorAgent.supported_models().await {
                    if !models.is_empty() {
                        *slot.lock().unwrap() = Some(models);
                    }
                }
            });
        }

        let screen = if let Some(number) = pr_number {
            // Go directly to comment list.
            let pr = github.get_pr(number).await?;
            let threads = github
                .get_review_threads(&repo_info.owner, &repo_info.name, number)
                .await?;
            Screen::CommentList(CommentListState::new(pr, &threads))
        } else {
            Screen::Splash {
                shown_at: Instant::now(),
            }
        };

        Ok(Self {
            config,
            github,
            agent,
            screen,
            popup: Popup::None,
            selected_model,
            cached_models: fallback,
            bg_models,
            repo_owner: repo_info.owner,
            repo_name: repo_info.name,
            should_quit: false,
        })
    }

    /// If the background model-fetch task has completed, swap in the live list.
    fn poll_bg_models(&mut self) {
        if let Ok(mut slot) = self.bg_models.try_lock() {
            if let Some(models) = slot.take() {
                self.cached_models = models;
                // Re-validate the selected model against the fresh list.
                if !self.cached_models.contains(&self.selected_model) {
                    if let Some(first) = self.cached_models.first() {
                        self.selected_model = first.clone();
                    }
                }
            }
        }
    }

    // ── Main loop ─────────────────────────────────────────────────────

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            // Check if the background model fetch has completed.
            self.poll_bg_models();

            // Draw.
            terminal.draw(|frame| self.render(frame))?;

            if self.should_quit {
                break;
            }

            // Handle automatic transitions (splash → next screen).
            if let Screen::Splash { shown_at } = &self.screen {
                let elapsed = shown_at.elapsed();
                let splash_dur =
                    Duration::from_millis(self.config.ui.splash_duration_ms);

                if elapsed >= splash_dur {
                    self.transition_from_splash().await?;
                    continue;
                }

                // Poll with a short timeout so we can transition once time is up.
                let remaining = splash_dur - elapsed;
                if event::poll(remaining.min(Duration::from_millis(50)))? {
                    if let Event::Key(key) = event::read()? {
                        // Any key press skips the splash.
                        if key.kind == event::KeyEventKind::Press {
                            self.transition_from_splash().await?;
                        }
                    }
                }
                continue;
            }

            // Normal event handling.
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != event::KeyEventKind::Press {
                        continue;
                    }
                    self.handle_key(key.code, key.modifiers).await?;
                }
            }
        }

        Ok(())
    }

    // ── Rendering ─────────────────────────────────────────────────────

    fn render(&mut self, frame: &mut ratatui::Frame) {
        match &mut self.screen {
            Screen::Splash { .. } => splash::render(frame),
            Screen::PrList(state) => ui::pr_list::render(frame, state),
            Screen::CommentList(state) => ui::comment_list::render(frame, state),
            Screen::CommentDetail { entry, .. } => {
                ui::comment_detail::render(frame, entry);
            }
        }

        // Render popup overlay.
        if let Popup::ModelSelector(ref mut state) = self.popup {
            ui::render_model_selector(frame, state);
        }
    }

    // ── Key handling ──────────────────────────────────────────────────

    async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        // Ctrl+C always quits.
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return Ok(());
        }

        // If a popup is open, route keys to it.
        if matches!(self.popup, Popup::ModelSelector(_)) {
            return self.handle_popup_key(code);
        }

        match &self.screen {
            Screen::Splash { .. } => {} // handled in the loop above
            Screen::PrList(_) => self.handle_pr_list_key(code).await?,
            Screen::CommentList(_) => self.handle_comment_list_key(code).await?,
            Screen::CommentDetail { .. } => {
                self.handle_comment_detail_key(code).await?;
            }
        }

        Ok(())
    }

    // ── PR list keys ──────────────────────────────────────────────────

    async fn handle_pr_list_key(&mut self, code: KeyCode) -> Result<()> {
        let Screen::PrList(ref mut state) = self.screen else {
            return Ok(());
        };

        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Down | KeyCode::Char('j') => state.next(),
            KeyCode::Up | KeyCode::Char('k') => state.previous(),
            KeyCode::Enter => {
                if let Some(pr) = state.selected_pr().cloned() {
                    let threads = self
                        .github
                        .get_review_threads(&self.repo_owner, &self.repo_name, pr.number)
                        .await?;
                    self.screen = Screen::CommentList(CommentListState::new(pr, &threads));
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Comment list keys ─────────────────────────────────────────────

    async fn handle_comment_list_key(&mut self, code: KeyCode) -> Result<()> {
        // We need to take ownership of the screen temporarily for some actions.
        let Screen::CommentList(ref mut state) = self.screen else {
            return Ok(());
        };

        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                // Go back to PR list.
                let prs = self.github.list_open_prs("@me").await?;
                self.screen = Screen::PrList(PrListState::new(prs));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.clear_message();
                state.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.clear_message();
                state.previous();
            }
            KeyCode::Char(' ') => state.toggle_select(),
            KeyCode::Enter => {
                if let Some(entry) = state.current_entry().cloned() {
                    // Move into detail view, preserving comment list state.
                    let Screen::CommentList(cl_state) =
                        std::mem::replace(&mut self.screen, Screen::Splash { shown_at: Instant::now() })
                    else {
                        unreachable!();
                    };
                    self.screen = Screen::CommentDetail {
                        entry,
                        parent: Box::new(cl_state),
                    };
                }
            }
            KeyCode::Char('o') => {
                if let Some(entry) = state.current_entry() {
                    let _ = open::that(&entry.url);
                    state.set_message("Opened in browser", false);
                }
            }
            KeyCode::Char('t') => {
                if let Some(entry) = state.current_entry().cloned() {
                    match self
                        .github
                        .add_reaction(
                            &self.repo_owner,
                            &self.repo_name,
                            &entry.comment_id,
                            "THUMBS_UP",
                        )
                        .await
                    {
                        Ok(()) => state.set_message("👍 Reaction added!", false),
                        Err(e) => state.set_message(format!("Error: {e}"), true),
                    }
                }
            }
            KeyCode::Char('m') => {
                self.popup = Popup::ModelSelector(ModelSelectorState::new(
                    self.cached_models.clone(),
                    &self.selected_model,
                ));
            }
            KeyCode::Char('a') => {
                self.send_to_agent().await?;
            }
            _ => {}
        }
        Ok(())
    }

    // ── Comment detail keys ───────────────────────────────────────────

    async fn handle_comment_detail_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                // Go back to comment list.
                let Screen::CommentDetail { parent, .. } =
                    std::mem::replace(&mut self.screen, Screen::Splash { shown_at: Instant::now() })
                else {
                    unreachable!();
                };
                self.screen = Screen::CommentList(*parent);
            }
            KeyCode::Char('o') => {
                if let Screen::CommentDetail { entry, .. } = &self.screen {
                    let _ = open::that(&entry.url);
                }
            }
            KeyCode::Char('t') => {
                if let Screen::CommentDetail { entry, .. } = &self.screen {
                    let _ = self
                        .github
                        .add_reaction(
                            &self.repo_owner,
                            &self.repo_name,
                            &entry.comment_id,
                            "THUMBS_UP",
                        )
                        .await;
                }
            }
            KeyCode::Char('a') => {
                // Send the current detail comment to the agent.
                if let Screen::CommentDetail { entry, parent } = &self.screen {
                    let pr = &parent.pr;
                    let comment = ReviewComment {
                        id: entry.comment_id.clone(),
                        body: entry.body.clone(),
                        path: entry.path.clone(),
                        line: entry.line,
                        start_line: None,
                        diff_hunk: entry.diff_hunk.clone(),
                        author: entry.author.clone(),
                        created_at: String::new(),
                        url: entry.url.clone(),
                    };
                    let prompt = agent::build_prompt(pr, &[&comment]);
                    let cwd = env::current_dir()?;
                    let result = self
                        .agent
                        .execute(&prompt, Some(&self.selected_model), &cwd)
                        .await?;

                    // Return to the comment list with a status message.
                    let Screen::CommentDetail { parent, .. } = std::mem::replace(
                        &mut self.screen,
                        Screen::Splash {
                            shown_at: Instant::now(),
                        },
                    ) else {
                        unreachable!();
                    };
                    let mut cl_state = *parent;
                    if result.success {
                        cl_state.set_message("✅ Agent completed successfully!", false);
                    } else {
                        cl_state.set_message(
                            format!("❌ Agent failed: {}", result.message),
                            true,
                        );
                    }
                    self.screen = Screen::CommentList(cl_state);
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Popup keys ────────────────────────────────────────────────────

    fn handle_popup_key(&mut self, code: KeyCode) -> Result<()> {
        let Popup::ModelSelector(ref mut state) = self.popup else {
            return Ok(());
        };

        match code {
            KeyCode::Down | KeyCode::Char('j') => state.next(),
            KeyCode::Up | KeyCode::Char('k') => state.previous(),
            KeyCode::Enter => {
                if let Some(model) = state.selected_model() {
                    self.selected_model = model.to_owned();
                }
                self.popup = Popup::None;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.popup = Popup::None;
            }
            _ => {}
        }
        Ok(())
    }

    // ── Actions ───────────────────────────────────────────────────────

    /// Send the selected comments (or the current one) to the AI agent.
    async fn send_to_agent(&mut self) -> Result<()> {
        let Screen::CommentList(ref mut state) = self.screen else {
            return Ok(());
        };

        let entries = state.selected_entries();
        if entries.is_empty() {
            state.set_message("No comments to send", true);
            return Ok(());
        }

        // Build review comments from entries.
        let comments: Vec<ReviewComment> = entries
            .iter()
            .map(|e| ReviewComment {
                id: e.comment_id.clone(),
                body: e.body.clone(),
                path: e.path.clone(),
                line: e.line,
                start_line: None,
                diff_hunk: e.diff_hunk.clone(),
                author: e.author.clone(),
                created_at: String::new(),
                url: e.url.clone(),
            })
            .collect();

        let comment_refs: Vec<&ReviewComment> = comments.iter().collect();
        let prompt = agent::build_prompt(&state.pr, &comment_refs);

        let count = comments.len();
        state.set_message(
            format!("🚀 Sending {count} comment(s) to {}...", self.agent.name()),
            false,
        );

        let cwd = env::current_dir()?;
        let result = self
            .agent
            .execute(&prompt, Some(&self.selected_model), &cwd)
            .await?;

        if result.success {
            state.set_message("✅ Agent completed successfully!", false);
        } else {
            state.set_message(format!("❌ Agent failed: {}", result.message), true);
        }

        Ok(())
    }

    // ── Splash transition ─────────────────────────────────────────────

    async fn transition_from_splash(&mut self) -> Result<()> {
        let branch = git::current_branch()?;

        // Check if there's an open PR for the current branch.
        if let Some(pr) = self.github.find_pr_for_branch(&branch).await? {
            let threads = self
                .github
                .get_review_threads(&self.repo_owner, &self.repo_name, pr.number)
                .await?;
            self.screen = Screen::CommentList(CommentListState::new(pr, &threads));
        } else {
            // Fall back to listing all PRs by the current user.
            let prs = self.github.list_open_prs("@me").await?;
            self.screen = Screen::PrList(PrListState::new(prs));
        }

        Ok(())
    }
}
