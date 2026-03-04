use std::time::Instant;

use anyhow::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use crate::github::provider::GitProvider;
use crate::ui::additional_instructions::AdditionalInstructionsState;
use crate::ui::comment_list::CommentListState;
use crate::ui::pr_list::PrListState;
use crate::ui::reply::ReplyState;
use crate::ui::ModelSelectorState;

use super::{AgentDispatchTarget, App, Popup, Screen};

impl App {
    pub(super) async fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<()> {
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return Ok(());
        }

        match &self.popup {
            Popup::ModelSelector(_) => return self.handle_popup_key(code),
            Popup::Reply(_) => return self.handle_reply_key(code, modifiers).await,
            Popup::AdditionalInstructions(_) => {
                return self
                    .handle_additional_instructions_key(code, modifiers)
                    .await;
            }
            Popup::None => {}
        }

        match &self.screen {
            Screen::Splash { .. } => {}
            Screen::PrList(_) => self.handle_pr_list_key(code).await?,
            Screen::CommentList(_) => {
                self.handle_comment_list_key(code, modifiers).await?;
            }
            Screen::CommentDetail { .. } => {
                self.handle_comment_detail_key(code, modifiers).await?;
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

    async fn handle_comment_list_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<()> {
        let running = self.running_comment_ids();

        let Screen::CommentList(ref mut state) = self.screen else {
            return Ok(());
        };

        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                let prs = self.github.list_open_prs("@me").await?;
                self.screen = Screen::PrList(PrListState::new(prs));
            }
            KeyCode::Char('l') => {
                self.show_agent_panel = !self.show_agent_panel;
                if self.show_agent_panel && !self.agent_jobs.is_empty() {
                    self.selected_agent_job = self.agent_jobs.len() - 1;
                    self.clear_selected_agent_unread();
                }
            }
            KeyCode::Char('v') => {
                self.output_mode = self.output_mode.toggle();
            }
            KeyCode::Char(']') => self.select_next_agent_job(),
            KeyCode::Char('[') => self.select_prev_agent_job(),
            KeyCode::Down | KeyCode::Char('j') => {
                state.clear_message();
                state.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.clear_message();
                state.previous();
            }
            KeyCode::Char(' ') => state.toggle_select(&running),
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                state.select_all(&running);
            }
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                state.deselect_all();
            }
            KeyCode::Enter => {
                if let Some(entry) = state.current_entry().cloned() {
                    let Screen::CommentList(cl_state) = std::mem::replace(
                        &mut self.screen,
                        Screen::Splash {
                            shown_at: Instant::now(),
                        },
                    ) else {
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
                        Ok(()) => {
                            state.mark_reacted(&entry.comment_id);
                            state.set_message("👍 Reaction added!", false);
                        }
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
                self.popup = Popup::AdditionalInstructions(AdditionalInstructionsState::new(
                    AgentDispatchTarget::CommentList,
                ));
            }
            KeyCode::Char('r') => {
                if let Some(entry) = state.current_entry().cloned() {
                    self.popup = Popup::Reply(ReplyState::new(
                        entry.thread_id.clone(),
                        state.pr.number,
                        entry.path.clone(),
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Comment detail keys ───────────────────────────────────────────

    async fn handle_comment_detail_key(
        &mut self,
        code: KeyCode,
        _modifiers: KeyModifiers,
    ) -> Result<()> {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                let Screen::CommentDetail { parent, .. } = std::mem::replace(
                    &mut self.screen,
                    Screen::Splash {
                        shown_at: Instant::now(),
                    },
                ) else {
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
                let comment_id = if let Screen::CommentDetail { entry, .. } = &self.screen {
                    entry.comment_id.clone()
                } else {
                    return Ok(());
                };
                let result = self
                    .github
                    .add_reaction(&self.repo_owner, &self.repo_name, &comment_id, "THUMBS_UP")
                    .await;
                if let Screen::CommentDetail { parent, .. } = &mut self.screen {
                    match result {
                        Ok(()) => {
                            parent.mark_reacted(&comment_id);
                            parent.set_message("👍 Reaction added!".to_owned(), false);
                        }
                        Err(e) => parent.set_message(format!("Error: {e}"), true),
                    }
                }
            }
            KeyCode::Char('a') => {
                self.popup = Popup::AdditionalInstructions(AdditionalInstructionsState::new(
                    AgentDispatchTarget::CommentDetail,
                ));
            }
            KeyCode::Char('r') => {
                if let Screen::CommentDetail { entry, parent } = &self.screen {
                    self.popup = Popup::Reply(ReplyState::new(
                        entry.thread_id.clone(),
                        parent.pr.number,
                        entry.path.clone(),
                    ));
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
            KeyCode::Esc => {
                self.popup = Popup::None;
            }
            KeyCode::Backspace => state.pop_filter_char(),
            KeyCode::Char(c) => state.push_filter_char(c),
            _ => {}
        }
        Ok(())
    }

    // ── Reply popup keys ──────────────────────────────────────────────

    async fn handle_reply_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if code == KeyCode::Esc {
            self.popup = Popup::None;
            return Ok(());
        }

        if code == KeyCode::Char('s') && modifiers.contains(KeyModifiers::CONTROL) {
            return self.submit_reply().await;
        }

        let Popup::Reply(ref mut state) = self.popup else {
            return Ok(());
        };
        state.handle_input(code, modifiers);

        Ok(())
    }

    pub(super) async fn handle_additional_instructions_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<()> {
        if code == KeyCode::Esc {
            self.popup = Popup::None;
            return Ok(());
        }

        if code == KeyCode::Char('s') && modifiers.contains(KeyModifiers::CONTROL) {
            return self.submit_additional_instructions().await;
        }

        let Popup::AdditionalInstructions(ref mut state) = self.popup else {
            return Ok(());
        };
        state.handle_input(code, modifiers);
        Ok(())
    }
}
