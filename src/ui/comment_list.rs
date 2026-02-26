use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::github::types::{PullRequest, ReviewThread};

use super::status_bar::{self, KeyHint};
use super::theme;

/// Flattened view of a review thread's root comment, used for display.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CommentEntry {
    /// Index into the parent `ReviewThread` list.
    pub thread_idx: usize,
    pub path: String,
    pub line: Option<u32>,
    pub author: String,
    pub body: String,
    pub diff_hunk: String,
    pub url: String,
    pub comment_id: String,
}

/// State for the comment list screen.
pub struct CommentListState {
    pub pr: PullRequest,
    pub entries: Vec<CommentEntry>,
    pub list_state: ListState,
    /// Indices of selected (checked) entries.
    pub selected: Vec<bool>,
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
                t.root_comment().map(|c| CommentEntry {
                    thread_idx: idx,
                    path: c.path.clone(),
                    line: c.line,
                    author: c.author.clone(),
                    body: c.body.clone(),
                    diff_hunk: c.diff_hunk.clone(),
                    url: c.url.clone(),
                    comment_id: c.id.clone(),
                })
            })
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
            message: None,
        }
    }

    pub fn next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 1) % self.entries.len()));
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
    pub fn toggle_select(&mut self) {
        if let Some(i) = self.list_state.selected() {
            self.selected[i] = !self.selected[i];
        }
    }

    /// Return the currently highlighted entry.
    pub fn current_entry(&self) -> Option<&CommentEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.entries.get(i))
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
}

// ── Rendering ─────────────────────────────────────────────────────────────

/// Render the comment list screen.
pub fn render(frame: &mut Frame, state: &mut CommentListState) {
    let area = frame.area();

    let [header_area, main_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(frame, header_area, state);

    // Split the main area: top for the comment list, bottom for code context.
    let [comments_area, context_area] = Layout::vertical([
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ])
    .areas(main_area);

    render_comment_list(frame, comments_area, state);
    render_code_context(frame, context_area, state);
    render_status_bar(frame, status_area, state);
}

fn render_header(frame: &mut Frame, area: Rect, state: &CommentListState) {
    let pr = &state.pr;
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" 🙏 PRAI", theme::accent()),
        Span::styled("  │  ", theme::border()),
        Span::styled(format!("PR #{}: ", pr.number), theme::accent()),
        Span::styled(&pr.title, theme::text()),
        Span::styled(
            format!("  ({})", pr.head_ref_name),
            theme::subtext(),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme::border()),
    )
    .style(theme::text());

    frame.render_widget(header, area);
}

fn render_comment_list(frame: &mut Frame, area: Rect, state: &mut CommentListState) {
    let unresolved = state.entries.len();
    let selected_count = state.selected_count();

    let title = if selected_count > 0 {
        format!(" Comments ({unresolved} unresolved, {selected_count} selected) ")
    } else {
        format!(" Comments ({unresolved} unresolved) ")
    };

    let items: Vec<ListItem<'_>> = state
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let checkbox = if state.selected[i] { "☑ " } else { "☐ " };
            let location = if let Some(line) = entry.line {
                format!("{}:{line}", entry.path)
            } else {
                entry.path.clone()
            };

            // Truncate the comment body to a single line.
            let body_preview: String = entry
                .body
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect();

            let line = Line::from(vec![
                Span::styled(checkbox, theme::checkbox()),
                Span::styled(format!("{location:<40}"), theme::text()),
                Span::styled(format!("@{:<14}", entry.author), theme::author()),
                Span::styled(body_preview, theme::subtext()),
            ]);

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

    let paragraph = Paragraph::new(content).block(block).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, state: &CommentListState) {
    // Show a transient message if one is set.
    if let Some((ref msg, is_error)) = state.message {
        let style = if is_error {
            theme::error()
        } else {
            theme::success()
        };
        let bar = Paragraph::new(Line::styled(format!(" {msg}"), style))
            .style(theme::status_bar());
        frame.render_widget(bar, area);
        return;
    }

    status_bar::render(
        frame,
        area,
        &[
            KeyHint { key: "Space", desc: "Select" },
            KeyHint { key: "a", desc: "Send to agent" },
            KeyHint { key: "m", desc: "Model" },
            KeyHint { key: "r", desc: "Reply" },
            KeyHint { key: "o", desc: "Browser" },
            KeyHint { key: "t", desc: "👍" },
            KeyHint { key: "q", desc: "Back" },
        ],
    );
}

// ── Need Rect in scope ────────────────────────────────────────────────────

use ratatui::layout::Rect;
