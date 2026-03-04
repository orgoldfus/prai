use std::env;
use std::time::Instant;

use anyhow::Result;

use crate::agent::cursor::CursorAgent;
use crate::agent::stream::AgentStreamEvent;
use crate::agent::{self};
use crate::git;
use crate::github::provider::GitProvider;
use crate::github::types::ReviewComment;
use crate::ui::agent_timeline::AgentTimeline;
use crate::ui::comment_list::{AgentJobStatus, CommentEntry, CommentListState, ThreadReply};
use crate::ui::pr_list::PrListState;

use super::{AgentDispatchTarget, AgentJob, App, Popup, Screen};

impl App {
    /// Send selected comments to the agent as a background task.
    pub(super) fn send_to_agent(&mut self, additional_instructions: Option<&str>) -> Result<()> {
        let running = self.running_comment_ids();

        let Screen::CommentList(ref mut state) = self.screen else {
            return Ok(());
        };

        let entries = state.selected_entries();

        let entries: Vec<&CommentEntry> = entries
            .into_iter()
            .filter(|e| !running.contains(&e.comment_id))
            .collect();

        if entries.is_empty() {
            state.set_message("Already running for selected comments", true);
            return Ok(());
        }

        let comment_ids: Vec<String> = entries.iter().map(|e| e.comment_id.clone()).collect();
        let comments: Vec<ReviewComment> = entries
            .iter()
            .map(|e| {
                ReviewComment::from_entry(
                    &e.comment_id,
                    &e.body,
                    &e.path,
                    e.line,
                    &e.diff_hunk,
                    &e.author,
                    &e.url,
                )
            })
            .collect();

        let comment_refs: Vec<&ReviewComment> = comments.iter().collect();
        let prompt =
            agent::build_prompt_with_additional(&state.pr, &comment_refs, additional_instructions);

        let count = comments.len();
        state.set_message(format!("🚀 Sent {count} comment(s) to agent"), false);

        self.spawn_agent_job(&prompt, comment_ids)?;

        Ok(())
    }

    /// Send the currently viewed detail comment to the agent.
    pub(super) fn send_detail_to_agent(
        &mut self,
        additional_instructions: Option<&str>,
    ) -> Result<()> {
        let running = self.running_comment_ids();

        let Screen::CommentDetail { entry, parent } = &self.screen else {
            return Ok(());
        };

        if running.contains(&entry.comment_id) {
            return Ok(());
        }

        let comment = ReviewComment::from_entry(
            &entry.comment_id,
            &entry.body,
            &entry.path,
            entry.line,
            &entry.diff_hunk,
            &entry.author,
            &entry.url,
        );
        let prompt =
            agent::build_prompt_with_additional(&parent.pr, &[&comment], additional_instructions);
        let comment_ids = vec![entry.comment_id.clone()];

        let Screen::CommentDetail { parent, .. } = std::mem::replace(
            &mut self.screen,
            Screen::Splash {
                shown_at: Instant::now(),
            },
        ) else {
            unreachable!();
        };
        let mut cl_state = *parent;
        cl_state.set_message("🚀 Sent comment to agent", false);
        self.screen = Screen::CommentList(cl_state);

        self.spawn_agent_job(&prompt, comment_ids)?;

        Ok(())
    }

    /// Spawn a background agent task and track it as a job.
    fn spawn_agent_job(&mut self, prompt: &str, comment_ids: Vec<String>) -> Result<()> {
        let model = self.selected_model.clone();
        let cwd = env::current_dir()?;

        let (stream_tx, stream_rx) = tokio::sync::mpsc::unbounded_channel();
        let run_model = model.clone();
        let owned_prompt = prompt.to_owned();
        let handle = tokio::spawn(async move {
            CursorAgent::execute_with_stream(&owned_prompt, Some(&run_model), &cwd, stream_tx)
                .await
        });

        let job_id = self.next_agent_job_id;
        self.next_agent_job_id += 1;
        let mut timeline = AgentTimeline::new(
            Self::MAX_JOB_LOG_LINES,
            Self::MAX_JOB_TIMELINE_NODES,
            Self::MAX_JOB_TIMELINE_CHARS,
        );
        timeline.apply_event(AgentStreamEvent::Info("agent started".to_owned()));
        self.agent_jobs.push(AgentJob {
            id: job_id,
            model,
            comment_ids,
            started_at: Instant::now(),
            finished_at: None,
            status: AgentJobStatus::Running,
            handle: Some(handle),
            stream_rx,
            timeline,
            unread_lines: 0,
        });
        self.selected_agent_job = self.agent_jobs.len() - 1;
        if self.show_agent_panel {
            self.clear_selected_agent_unread();
        }

        Ok(())
    }

    pub(super) async fn submit_reply(&mut self) -> Result<()> {
        let Popup::Reply(ref state) = self.popup else {
            return Ok(());
        };

        let body = state.text();
        if body.trim().is_empty() {
            return Ok(());
        }

        let thread_id = state.thread_id.clone();
        let body = body.to_owned();

        self.popup = Popup::None;

        match self.github.reply_to_thread(&thread_id, &body).await {
            Ok(()) => {
                self.set_screen_message("💬 Reply posted!".to_owned(), false);
                self.add_reply_to_screen(&thread_id, "you".to_owned(), body);
            }
            Err(e) => self.set_screen_message(format!("Error: {e}"), true),
        }

        Ok(())
    }

    pub(super) async fn submit_additional_instructions(&mut self) -> Result<()> {
        let Popup::AdditionalInstructions(ref state) = self.popup else {
            return Ok(());
        };

        let raw_text = state.text();
        let target = state.target;
        let extra_owned = raw_text.trim().to_owned();
        let extra = if extra_owned.is_empty() {
            None
        } else {
            Some(extra_owned.as_str())
        };

        self.popup = Popup::None;

        match target {
            AgentDispatchTarget::CommentList => self.send_to_agent(extra)?,
            AgentDispatchTarget::CommentDetail => self.send_detail_to_agent(extra)?,
        }

        Ok(())
    }

    pub(super) fn add_reply_to_screen(
        &mut self,
        thread_id: &str,
        author: String,
        body: String,
    ) {
        match &mut self.screen {
            Screen::CommentList(state) => {
                state.add_reply_to_thread(thread_id, author, body);
            }
            Screen::CommentDetail { entry, parent } => {
                if entry.thread_id == thread_id {
                    entry.replies.push(ThreadReply {
                        author: author.clone(),
                        body: body.clone(),
                        created_at: String::new(),
                    });
                }
                parent.add_reply_to_thread(thread_id, author, body);
            }
            _ => {}
        }
    }

    pub(super) async fn transition_from_splash(&mut self) -> Result<()> {
        let branch = git::current_branch()?;

        if let Some(pr) = self.github.find_pr_for_branch(&branch).await? {
            let threads = self
                .github
                .get_review_threads(&self.repo_owner, &self.repo_name, pr.number)
                .await?;
            self.screen = Screen::CommentList(CommentListState::new(pr, &threads));
        } else {
            let prs = self.github.list_open_prs("@me").await?;
            self.screen = Screen::PrList(PrListState::new(prs));
        }

        Ok(())
    }
}
