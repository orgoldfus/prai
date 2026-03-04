mod actions;
mod keys;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event};
use ratatui::DefaultTerminal;
use throbber_widgets_tui::ThrobberState;
use tokio::task::JoinHandle;

use crate::agent::cursor::CursorAgent;
use crate::agent::provider::{AgentProvider, AgentResult};
use crate::agent::stream::{parse_stream_chunk, StreamChunk};
use crate::config::Config;
use crate::git;
use crate::github::client::GitHubClient;
use crate::github::provider::GitProvider;
use crate::ui::additional_instructions::AdditionalInstructionsState;
use crate::ui::agent_timeline::{AgentOutputMode, AgentTimeline};
use crate::ui::comment_list::{
    AgentJobStatus, AgentJobSummary, AgentPanelView, CommentEntry, CommentListState,
};
use crate::ui::pr_list::PrListState;
use crate::ui::reply::ReplyState;
use crate::ui::{self, splash, ModelSelectorState};

// ── Screens ───────────────────────────────────────────────────────────────

/// Which screen the app is currently showing.
enum Screen {
    Splash { shown_at: Instant },
    PrList(PrListState),
    CommentList(CommentListState),
    CommentDetail {
        entry: CommentEntry,
        parent: Box<CommentListState>,
    },
}

/// Optional overlay popup on top of the current screen.
enum Popup {
    None,
    ModelSelector(ModelSelectorState),
    Reply(ReplyState),
    AdditionalInstructions(AdditionalInstructionsState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentDispatchTarget {
    CommentList,
    CommentDetail,
}

// ── Agent jobs ────────────────────────────────────────────────────────────

struct AgentJob {
    id: u64,
    model: String,
    comment_ids: Vec<String>,
    started_at: Instant,
    finished_at: Option<Instant>,
    status: AgentJobStatus,
    handle: Option<JoinHandle<Result<AgentResult>>>,
    stream_rx: tokio::sync::mpsc::UnboundedReceiver<StreamChunk>,
    timeline: AgentTimeline,
    unread_lines: usize,
}

// ── App ───────────────────────────────────────────────────────────────────

pub struct App {
    config: Config,
    github: GitHubClient,
    screen: Screen,
    popup: Popup,
    selected_model: String,
    cached_models: Vec<String>,
    bg_models: Arc<Mutex<Option<Vec<String>>>>,
    repo_owner: String,
    repo_name: String,
    agent_jobs: Vec<AgentJob>,
    handled_comment_ids: HashSet<String>,
    next_agent_job_id: u64,
    show_agent_panel: bool,
    selected_agent_job: usize,
    output_mode: AgentOutputMode,
    animation_started_at: Instant,
    pub throbber_state: ThrobberState,
    should_quit: bool,
}

impl App {
    const MAX_JOB_LOG_LINES: usize = 1200;
    const MAX_JOB_TIMELINE_NODES: usize = 600;
    const MAX_JOB_TIMELINE_CHARS: usize = 24_000;

    pub async fn new(config: Config, pr_number: Option<u64>) -> Result<Self> {
        let github = GitHubClient;

        let repo_info = git::repo_info().context("failed to read git remote")?;

        let fallback = CursorAgent::fallback_models();
        let selected_model = config.agent.default_model.clone();

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
            screen,
            popup: Popup::None,
            selected_model,
            cached_models: fallback,
            bg_models,
            repo_owner: repo_info.owner,
            repo_name: repo_info.name,
            agent_jobs: Vec::new(),
            handled_comment_ids: HashSet::new(),
            next_agent_job_id: 1,
            show_agent_panel: false,
            selected_agent_job: 0,
            output_mode: AgentOutputMode::Ui,
            animation_started_at: Instant::now(),
            throbber_state: ThrobberState::default(),
            should_quit: false,
        })
    }

    // ── Polling ────────────────────────────────────────────────────────

    fn poll_bg_models(&mut self) {
        if let Ok(mut slot) = self.bg_models.try_lock() {
            if let Some(models) = slot.take() {
                self.cached_models = models;
                if !self.cached_models.contains(&self.selected_model) {
                    if let Some(first) = self.cached_models.first() {
                        self.selected_model = first.clone();
                    }
                }
            }
        }
    }

    fn running_comment_ids(&self) -> HashSet<String> {
        self.agent_jobs
            .iter()
            .filter(|j| j.status == AgentJobStatus::Running)
            .flat_map(|j| j.comment_ids.iter().cloned())
            .collect()
    }

    fn poll_agent_jobs(&mut self) {
        for idx in 0..self.agent_jobs.len() {
            let is_selected = self.selected_agent_job_index() == Some(idx) && self.show_agent_panel;
            while let Ok(chunk) = self.agent_jobs[idx].stream_rx.try_recv() {
                self.agent_jobs[idx]
                    .timeline
                    .push_raw_line(chunk.raw_line());
                if let Some(event) = parse_stream_chunk(&chunk) {
                    self.agent_jobs[idx].timeline.apply_event(event);
                }
                if !is_selected {
                    self.agent_jobs[idx].unread_lines += 1;
                }
            }
        }

        let mut finished = Vec::new();
        for (idx, job) in self.agent_jobs.iter().enumerate() {
            if job.status == AgentJobStatus::Running
                && job.handle.as_ref().is_some_and(|h| h.is_finished())
            {
                finished.push(idx);
            }
        }

        for idx in finished {
            let count = self.agent_jobs[idx].comment_ids.len();
            let result = self.agent_jobs[idx]
                .handle
                .take()
                .and_then(|h| h.now_or_never());

            match result {
                Some(Ok(Ok(r))) if r.success => {
                    self.agent_jobs[idx].status = AgentJobStatus::Success;
                    self.agent_jobs[idx].finished_at = Some(Instant::now());
                    self.agent_jobs[idx].timeline.mark_complete(true, None);
                    let handled_ids = self.agent_jobs[idx].comment_ids.clone();
                    self.handled_comment_ids.extend(handled_ids);
                    self.set_screen_message(
                        format!("✅ Agent completed ({count} comment(s))"),
                        false,
                    );
                }
                Some(Ok(Ok(r))) => {
                    self.agent_jobs[idx].status = AgentJobStatus::Failed;
                    self.agent_jobs[idx].finished_at = Some(Instant::now());
                    self.agent_jobs[idx]
                        .timeline
                        .mark_complete(false, Some(&r.message));
                    self.set_screen_message(format!("❌ Agent failed: {}", r.message), true);
                }
                Some(Ok(Err(e))) => {
                    self.agent_jobs[idx].status = AgentJobStatus::Failed;
                    self.agent_jobs[idx].finished_at = Some(Instant::now());
                    self.agent_jobs[idx]
                        .timeline
                        .mark_complete(false, Some(&e.to_string()));
                    self.set_screen_message(format!("❌ Agent error: {e}"), true);
                }
                Some(Err(e)) => {
                    self.agent_jobs[idx].status = AgentJobStatus::Failed;
                    self.agent_jobs[idx].finished_at = Some(Instant::now());
                    self.agent_jobs[idx]
                        .timeline
                        .mark_complete(false, Some(&e.to_string()));
                    self.set_screen_message(format!("❌ Agent panic: {e}"), true);
                }
                None => {}
            }
        }
    }

    // ── Agent job navigation ──────────────────────────────────────────

    fn selected_agent_job_index(&self) -> Option<usize> {
        if self.agent_jobs.is_empty() {
            None
        } else {
            Some(self.selected_agent_job.min(self.agent_jobs.len() - 1))
        }
    }

    fn clear_selected_agent_unread(&mut self) {
        if let Some(idx) = self.selected_agent_job_index() {
            self.agent_jobs[idx].unread_lines = 0;
        }
    }

    fn select_next_agent_job(&mut self) {
        if self.agent_jobs.is_empty() {
            return;
        }
        self.selected_agent_job = (self.selected_agent_job + 1) % self.agent_jobs.len();
        self.clear_selected_agent_unread();
    }

    fn select_prev_agent_job(&mut self) {
        if self.agent_jobs.is_empty() {
            return;
        }
        self.selected_agent_job = self
            .selected_agent_job
            .checked_sub(1)
            .unwrap_or(self.agent_jobs.len() - 1);
        self.clear_selected_agent_unread();
    }

    fn set_screen_message(&mut self, msg: String, is_error: bool) {
        match &mut self.screen {
            Screen::CommentList(state) => state.set_message(msg, is_error),
            Screen::CommentDetail { parent, .. } => parent.set_message(msg, is_error),
            _ => {}
        }
    }

    // ── Main loop ─────────────────────────────────────────────────────

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            self.poll_bg_models();
            self.poll_agent_jobs();
            self.throbber_state.calc_next();

            terminal.draw(|frame| self.render(frame))?;

            if self.should_quit {
                break;
            }

            if let Screen::Splash { shown_at } = &self.screen {
                let elapsed = shown_at.elapsed();
                let splash_dur = Duration::from_millis(self.config.ui.splash_duration_ms);

                if elapsed >= splash_dur {
                    self.transition_from_splash().await?;
                    continue;
                }

                let remaining = splash_dur - elapsed;
                if event::poll(remaining.min(Duration::from_millis(50)))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == event::KeyEventKind::Press {
                            self.transition_from_splash().await?;
                        }
                    }
                }
                continue;
            }

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
        let running = self.running_comment_ids();
        let running_count = self
            .agent_jobs
            .iter()
            .filter(|j| j.status == AgentJobStatus::Running)
            .count();
        let pulse_on = (self.animation_started_at.elapsed().as_millis() / 450).is_multiple_of(2);

        let agent_panel = AgentPanelView {
            visible: self.show_agent_panel,
            selected_idx: self.selected_agent_job_index(),
            output_mode: self.output_mode,
            pulse_on,
            jobs: self
                .agent_jobs
                .iter()
                .map(|job| AgentJobSummary {
                    id: job.id,
                    model: &job.model,
                    comment_count: job.comment_ids.len(),
                    status: job.status,
                    unread_lines: job.unread_lines,
                    elapsed: job
                        .finished_at
                        .unwrap_or_else(Instant::now)
                        .duration_since(job.started_at),
                })
                .collect(),
            selected_timeline: self
                .selected_agent_job_index()
                .and_then(|idx| self.agent_jobs.get(idx))
                .map(|j| &j.timeline),
        };

        match &mut self.screen {
            Screen::Splash { .. } => splash::render(frame),
            Screen::PrList(state) => ui::pr_list::render(frame, state),
            Screen::CommentList(state) => {
                ui::comment_list::render(
                    frame,
                    state,
                    &running,
                    &self.handled_comment_ids,
                    &self.throbber_state,
                    running_count,
                    &agent_panel,
                );
            }
            Screen::CommentDetail { entry, .. } => {
                ui::comment_detail::render(frame, entry);
            }
        }

        match &mut self.popup {
            Popup::ModelSelector(ref mut state) => {
                ui::render_model_selector(frame, state);
            }
            Popup::Reply(ref state) => {
                ui::reply::render(frame, state);
            }
            Popup::AdditionalInstructions(ref state) => {
                ui::additional_instructions::render(frame, state);
            }
            Popup::None => {}
        }
    }
}

/// Extension to get a result from a finished JoinHandle without async.
trait JoinHandleExt<T> {
    fn now_or_never(self) -> Option<std::result::Result<T, tokio::task::JoinError>>;
}

impl<T> JoinHandleExt<T> for JoinHandle<T> {
    fn now_or_never(self) -> Option<std::result::Result<T, tokio::task::JoinError>> {
        if self.is_finished() {
            Some(futures::executor::block_on(self))
        } else {
            None
        }
    }
}
