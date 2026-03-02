use std::collections::HashSet;
use std::time::Duration;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use throbber_widgets_tui::ThrobberState;

use crate::github::types::{PullRequest, ReviewThread};

use super::agent_timeline::{AgentOutputMode, AgentTimeline};
use super::status_bar::{self, KeyHint};
use super::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentJobStatus {
    Running,
    Success,
    Failed,
}

pub struct AgentJobSummary<'a> {
    pub id: u64,
    pub model: &'a str,
    pub comment_count: usize,
    pub status: AgentJobStatus,
    pub unread_lines: usize,
    pub elapsed: Duration,
}

pub struct AgentPanelView<'a> {
    pub visible: bool,
    pub selected_idx: Option<usize>,
    pub output_mode: AgentOutputMode,
    pub pulse_on: bool,
    pub jobs: Vec<AgentJobSummary<'a>>,
    pub selected_timeline: Option<&'a AgentTimeline>,
}

/// A reply in a review thread (non-root comment).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ThreadReply {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

/// Flattened view of a review thread, used for display.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CommentEntry {
    /// Index into the parent `ReviewThread` list.
    pub thread_idx: usize,
    pub thread_id: String,
    pub path: String,
    pub line: Option<u32>,
    pub author: String,
    pub body: String,
    pub diff_hunk: String,
    pub url: String,
    pub comment_id: String,
    pub has_thumbs_up: bool,
    pub replies: Vec<ThreadReply>,
}

/// State for the comment list screen.
pub struct CommentListState {
    pub pr: PullRequest,
    pub entries: Vec<CommentEntry>,
    pub list_state: ListState,
    /// Indices of selected (checked) entries.
    pub selected: Vec<bool>,
    /// Comment IDs that have been given a 👍 reaction this session.
    pub reacted: HashSet<String>,
    /// Transient status message shown at the bottom.
    pub message: Option<(String, bool)>, // (text, is_error)
}

impl CommentListState {
    /// Build from unresolved review threads.
    pub fn new(pr: PullRequest, threads: &[ReviewThread]) -> Self {
        let entries: Vec<CommentEntry> = threads
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_resolved)
            .filter_map(|(idx, t)| {
                t.root_comment().map(|c| {
                    let replies = t
                        .comments
                        .iter()
                        .skip(1)
                        .map(|r| ThreadReply {
                            author: r.author.clone(),
                            body: r.body.clone(),
                            created_at: r.created_at.clone(),
                        })
                        .collect();

                    CommentEntry {
                        thread_idx: idx,
                        thread_id: t.id.clone(),
                        path: c.path.clone(),
                        line: c.line,
                        author: c.author.clone(),
                        body: c.body.clone(),
                        diff_hunk: c.diff_hunk.clone(),
                        url: c.url.clone(),
                        comment_id: c.id.clone(),
                        has_thumbs_up: c.has_thumbs_up,
                        replies,
                    }
                })
            })
            .collect();

        let reacted: HashSet<String> = entries
            .iter()
            .filter(|e| e.has_thumbs_up)
            .map(|e| e.comment_id.clone())
            .collect();

        let count = entries.len();
        let mut list_state = ListState::default();
        if !entries.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            pr,
            entries,
            list_state,
            selected: vec![false; count],
            reacted,
            message: None,
        }
    }

    pub fn next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % self.entries.len()));
    }

    pub fn previous(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some(i.checked_sub(1).unwrap_or(self.entries.len() - 1)));
    }

    /// Toggle the selected state of the currently highlighted entry.
    /// Skips entries that are currently being processed by an agent.
    pub fn toggle_select(&mut self, running: &HashSet<String>) {
        if let Some(i) = self.list_state.selected() {
            if !running.contains(&self.entries[i].comment_id) {
                self.selected[i] = !self.selected[i];
            }
        }
    }

    /// Select all entries that are not currently running.
    pub fn select_all(&mut self, running: &HashSet<String>) {
        for (i, entry) in self.entries.iter().enumerate() {
            if !running.contains(&entry.comment_id) {
                self.selected[i] = true;
            }
        }
    }

    /// Deselect all entries.
    pub fn deselect_all(&mut self) {
        self.selected.fill(false);
    }

    /// Return the currently highlighted entry.
    pub fn current_entry(&self) -> Option<&CommentEntry> {
        self.list_state.selected().and_then(|i| self.entries.get(i))
    }

    /// Collect all selected entries (or the current one if none are selected).
    pub fn selected_entries(&self) -> Vec<&CommentEntry> {
        let selected: Vec<&CommentEntry> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(_, e)| e)
            .collect();

        if selected.is_empty() {
            // Fall back to the currently highlighted entry.
            self.current_entry().into_iter().collect()
        } else {
            selected
        }
    }

    pub fn selected_count(&self) -> usize {
        self.selected.iter().filter(|&&s| s).count()
    }

    pub fn set_message(&mut self, msg: impl Into<String>, is_error: bool) {
        self.message = Some((msg.into(), is_error));
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }

    pub fn mark_reacted(&mut self, comment_id: &str) {
        self.reacted.insert(comment_id.to_owned());
    }

    pub fn add_reply_to_thread(&mut self, thread_id: &str, author: String, body: String) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.thread_id == thread_id) {
            entry.replies.push(ThreadReply {
                author,
                body,
                created_at: String::new(),
            });
        }
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────

/// Render the comment list screen.
pub fn render(
    frame: &mut Frame,
    state: &mut CommentListState,
    running: &HashSet<String>,
    handled: &HashSet<String>,
    throbber: &ThrobberState,
    running_job_count: usize,
    agent_panel: &AgentPanelView<'_>,
) {
    let area = frame.area();

    let [header_area, main_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(frame, header_area, state);

    if agent_panel.visible {
        let [left_area, panel_area] =
            Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)])
                .areas(main_area);
        render_comments_and_context(frame, left_area, state, running, handled, throbber);
        render_agent_panel(frame, panel_area, agent_panel);
    } else {
        render_comments_and_context(frame, main_area, state, running, handled, throbber);
    }

    render_status_bar(frame, status_area, state, running_job_count, agent_panel);
}

fn render_comments_and_context(
    frame: &mut Frame,
    area: Rect,
    state: &mut CommentListState,
    running: &HashSet<String>,
    handled: &HashSet<String>,
    throbber: &ThrobberState,
) {
    let list_lines = state.entries.len() as u16 + 2;
    let max_list = (area.height * 50) / 100;
    let list_height = list_lines.clamp(4, max_list);

    let [comments_area, context_area] =
        Layout::vertical([Constraint::Length(list_height), Constraint::Min(4)]).areas(area);

    render_comment_list(frame, comments_area, state, running, handled, throbber);
    render_code_context(frame, context_area, state);
}

fn render_header(frame: &mut Frame, area: Rect, state: &CommentListState) {
    let pr = &state.pr;
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" 🙏 PRAI", theme::accent()),
        Span::styled("  │  ", theme::border()),
        Span::styled(format!("PR #{}: ", pr.number), theme::accent()),
        Span::styled(&pr.title, theme::text()),
        Span::styled(format!("  ({})", pr.head_ref_name), theme::subtext()),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme::border()),
    )
    .style(theme::text());

    frame.render_widget(header, area);
}

fn render_comment_list(
    frame: &mut Frame,
    area: Rect,
    state: &mut CommentListState,
    running: &HashSet<String>,
    handled: &HashSet<String>,
    throbber: &ThrobberState,
) {
    let unresolved = state.entries.len();
    let selected_count = state.selected_count();

    let title = if selected_count > 0 {
        format!(" Comments ({unresolved} unresolved, {selected_count} selected) ")
    } else {
        format!(" Comments ({unresolved} unresolved) ")
    };

    let idx = throbber.index().unsigned_abs() as usize;
    let spinner_symbol = throbber_widgets_tui::BRAILLE_SIX
        .symbols
        .get(idx % throbber_widgets_tui::BRAILLE_SIX.symbols.len())
        .copied()
        .unwrap_or("⠋");

    let items: Vec<ListItem<'_>> = state
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_running = running.contains(&entry.comment_id);
            let is_handled = handled.contains(&entry.comment_id);

            let prefix = if is_running {
                format!("{spinner_symbol} ")
            } else if state.selected[i] {
                "☑ ".to_owned()
            } else if is_handled {
                "✓ ".to_owned()
            } else {
                "☐ ".to_owned()
            };

            let text_style = if is_running {
                theme::subtext()
            } else {
                theme::text()
            };

            let location = if let Some(line) = entry.line {
                format!("{}:{line}", entry.path)
            } else {
                entry.path.clone()
            };

            let body_preview: String = entry
                .body
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect();

            let prefix_style = if is_running {
                theme::accent()
            } else if is_handled {
                theme::success()
            } else {
                theme::checkbox()
            };

            let mut spans = vec![
                Span::styled(prefix, prefix_style),
                Span::styled(format!("{location:<40}"), text_style),
                Span::styled(
                    format!("@{:<14}", entry.author),
                    if is_running {
                        theme::subtext()
                    } else {
                        theme::author()
                    },
                ),
                Span::styled(body_preview, theme::subtext()),
            ];

            if !entry.replies.is_empty() {
                spans.push(Span::styled(
                    format!("  💬 {}", entry.replies.len()),
                    theme::subtext(),
                ));
            }

            if state.reacted.contains(&entry.comment_id) {
                spans.push(Span::styled("  👍", theme::subtext()));
            }

            let line = Line::from(spans);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border()),
        )
        .highlight_style(theme::selected())
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_code_context(frame: &mut Frame, area: Rect, state: &CommentListState) {
    let content = if let Some(entry) = state.current_entry() {
        let mut lines: Vec<Line<'_>> = Vec::new();

        // Comment body.
        lines.push(Line::from(vec![
            Span::styled("💬 ", theme::accent()),
            Span::styled(format!("@{}", entry.author), theme::author()),
            Span::styled(": ", theme::text()),
        ]));
        for body_line in entry.body.lines() {
            lines.push(Line::styled(format!("   {body_line}"), theme::text()));
        }
        lines.push(Line::from(""));

        // Diff hunk with syntax colouring.
        if !entry.diff_hunk.is_empty() {
            lines.push(Line::styled("── Diff Context ──", theme::subtext()));
            for diff_line in entry.diff_hunk.lines() {
                let style = if diff_line.starts_with('+') {
                    theme::diff_add()
                } else if diff_line.starts_with('-') {
                    theme::diff_del()
                } else if diff_line.starts_with("@@") {
                    theme::subtext()
                } else {
                    theme::text()
                };
                lines.push(Line::styled(format!("  {diff_line}"), style));
            }
        }

        Text::from(lines)
    } else {
        Text::styled("No comment selected", theme::subtext())
    };

    let block = Block::default()
        .title(" Code Context ")
        .title_style(theme::accent())
        .borders(Borders::ALL)
        .border_style(theme::border());

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_agent_panel(frame: &mut Frame, area: Rect, panel: &AgentPanelView<'_>) {
    let jobs_height = (panel.jobs.len() as u16 + 2).clamp(4, (area.height * 45) / 100);
    let [jobs_area, logs_area] =
        Layout::vertical([Constraint::Length(jobs_height), Constraint::Min(4)]).areas(area);

    let mut list_state = ListState::default();
    list_state.select(panel.selected_idx);

    let items: Vec<ListItem<'_>> = panel
        .jobs
        .iter()
        .map(|job| {
            let status = match job.status {
                AgentJobStatus::Running => "⏳",
                AgentJobStatus::Success => "✅",
                AgentJobStatus::Failed => "❌",
            };
            let unread = if job.unread_lines > 0 {
                format!(" +{}", job.unread_lines)
            } else {
                String::new()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{status} #{} ", job.id), theme::accent()),
                Span::styled(
                    format!("{}c · {}s", job.comment_count, job.elapsed.as_secs()),
                    theme::text(),
                ),
                Span::styled(unread, theme::subtext()),
                Span::styled(format!(" · {}", job.model), theme::subtext()),
            ]))
        })
        .collect();

    let jobs = List::new(items)
        .block(
            Block::default()
                .title(" Agent Jobs ")
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border()),
        )
        .highlight_style(theme::selected())
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(jobs, jobs_area, &mut list_state);

    let output_rows: Vec<String> = if let Some(timeline) = panel.selected_timeline {
        match panel.output_mode {
            AgentOutputMode::Ui => timeline.ui_lines(panel.pulse_on),
            AgentOutputMode::Raw => timeline.raw_logs().iter().cloned().collect(),
        }
    } else if panel.jobs.is_empty() {
        vec!["No agent jobs yet".to_owned()]
    } else {
        vec!["Select a job to view output".to_owned()]
    };
    let lines: Vec<Line<'_>> = output_rows
        .iter()
        .map(|line| {
            let style = if line.starts_with("❌") {
                theme::error()
            } else if line.starts_with("✅") {
                theme::success()
            } else if line.starts_with("◉") || line.starts_with("○") || line.starts_with("◌")
            {
                theme::accent()
            } else if line.starts_with("•") {
                theme::subtext()
            } else {
                theme::text()
            };
            Line::styled(format!(" {line}"), style)
        })
        .collect();

    let output_title = match panel.output_mode {
        AgentOutputMode::Ui => " Agent Output (UI) ",
        AgentOutputMode::Raw => " Agent Output (Raw) ",
    };

    let logs = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title(output_title)
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border()),
        )
        .wrap(Wrap { trim: false })
        .scroll(log_scroll(output_rows.len(), logs_area));
    frame.render_widget(logs, logs_area);
}

fn log_scroll(total_rows: usize, area: Rect) -> (u16, u16) {
    let visible_lines = area.height.saturating_sub(2) as usize;
    if total_rows <= visible_lines {
        return (0, 0);
    }
    ((total_rows - visible_lines) as u16, 0)
}

fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    state: &CommentListState,
    running_job_count: usize,
    panel: &AgentPanelView<'_>,
) {
    // Show a transient message if one is set.
    if let Some((ref msg, is_error)) = state.message {
        let style = if is_error {
            theme::error()
        } else {
            theme::success()
        };

        let suffix = if running_job_count > 0 {
            format!("  ⏳ {running_job_count} agent job(s) running")
        } else {
            String::new()
        };

        let bar = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {msg}"), style),
            Span::styled(suffix, theme::accent()),
        ]))
        .style(theme::status_bar());
        frame.render_widget(bar, area);
        return;
    }

    let selected_count = state.selected_count();
    let agent_desc = if selected_count > 0 {
        format!("Send {selected_count} to agent")
    } else {
        "Send to agent".to_owned()
    };
    let running_desc = if running_job_count > 0 {
        Some(format!("⏳ {running_job_count} job(s)"))
    } else {
        None
    };
    let view_desc = match panel.output_mode {
        AgentOutputMode::Ui => "UI view",
        AgentOutputMode::Raw => "Raw view",
    };

    let mut hints: Vec<KeyHint<'_>> = vec![
        KeyHint {
            key: "Space",
            desc: "Select",
        },
        KeyHint {
            key: "^a",
            desc: "All",
        },
        KeyHint {
            key: "^d",
            desc: "None",
        },
        KeyHint {
            key: "a",
            desc: &agent_desc,
        },
        KeyHint {
            key: "l",
            desc: "Logs",
        },
        KeyHint {
            key: "v",
            desc: view_desc,
        },
        KeyHint {
            key: "m",
            desc: "Model",
        },
        KeyHint {
            key: "r",
            desc: "Reply",
        },
        KeyHint {
            key: "o",
            desc: "Browser",
        },
        KeyHint {
            key: "t",
            desc: "👍",
        },
        KeyHint {
            key: "q",
            desc: "Back",
        },
    ];

    if panel.visible && !panel.jobs.is_empty() {
        hints.push(KeyHint {
            key: "[/]",
            desc: "Job",
        });
    }

    if let Some(ref running_desc) = running_desc {
        hints.push(KeyHint {
            key: "",
            desc: running_desc,
        });
    }

    status_bar::render(frame, area, &hints);
}
