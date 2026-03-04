use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::github::types::PullRequest;

use super::status_bar::{self, KeyHint};
use super::theme;

/// State for the PR selection screen.
pub struct PrListState {
    pub prs: Vec<PullRequest>,
    pub list_state: ListState,
}

impl PrListState {
    pub fn new(prs: Vec<PullRequest>) -> Self {
        let mut list_state = ListState::default();
        if !prs.is_empty() {
            list_state.select(Some(0));
        }
        Self { prs, list_state }
    }

    pub fn selected_pr(&self) -> Option<&PullRequest> {
        self.list_state.selected().and_then(|i| self.prs.get(i))
    }

    pub fn next(&mut self) {
        if self.prs.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % self.prs.len()));
    }

    pub fn previous(&mut self) {
        if self.prs.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some(i.checked_sub(1).unwrap_or(self.prs.len() - 1)));
    }
}

/// Render the PR selection screen.
pub fn render(frame: &mut Frame, state: &mut PrListState) {
    let area = frame.area();

    let [header_area, list_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // ── Header ────────────────────────────────────────────────────────
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" 🙏 PRAI", theme::accent()),
        Span::styled("  │  ", theme::border()),
        Span::styled("Select a Pull Request", theme::text()),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme::border()),
    )
    .style(theme::text());

    frame.render_widget(header, header_area);

    // ── PR list ───────────────────────────────────────────────────────
    let items: Vec<ListItem<'_>> = state
        .prs
        .iter()
        .map(|pr| {
            let line = Line::from(vec![
                Span::styled(format!("#{:<6}", pr.number), theme::accent()),
                Span::styled(&pr.title, theme::text()),
                Span::styled(format!("  ({})", pr.head_ref_name), theme::subtext()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Open Pull Requests ")
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border()),
        )
        .highlight_style(theme::selected())
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, list_area, &mut state.list_state);

    // ── Status bar ──────────────────────────────────────────────────── 
    status_bar::render(
        frame,
        status_area,
        &[
            KeyHint {
                key: "↑↓",
                desc: "Navigate",
            },
            KeyHint {
                key: "Enter",
                desc: "Select",
            },
            KeyHint {
                key: "q",
                desc: "Quit",
            },
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::types::PrAuthor;

    fn make_pr(number: u64) -> PullRequest {
        PullRequest {
            number,
            title: format!("PR #{number}"),
            body: String::new(),
            url: format!("https://example.com/pr/{number}"),
            head_ref_name: "feature".to_owned(),
            base_ref_name: "main".to_owned(),
            created_at: String::new(),
            author: PrAuthor::default(),
        }
    }

    #[test]
    fn empty_list_navigation_is_safe() {
        let mut state = PrListState::new(vec![]);
        state.next();
        state.previous();
        assert_eq!(state.list_state.selected(), None);
        assert!(state.selected_pr().is_none());
    }

    #[test]
    fn new_selects_first() {
        let state = PrListState::new(vec![make_pr(1), make_pr(2)]);
        assert_eq!(state.list_state.selected(), Some(0));
    }

    #[test]
    fn next_wraps_around() {
        let mut state = PrListState::new(vec![make_pr(1), make_pr(2)]);
        state.next();
        assert_eq!(state.list_state.selected(), Some(1));
        state.next();
        assert_eq!(state.list_state.selected(), Some(0));
    }

    #[test]
    fn previous_wraps_around() {
        let mut state = PrListState::new(vec![make_pr(1), make_pr(2)]);
        state.previous();
        assert_eq!(state.list_state.selected(), Some(1));
    }
}
