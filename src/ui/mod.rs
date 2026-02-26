pub mod comment_detail;
pub mod comment_list;
pub mod pr_list;
pub mod splash;
pub mod status_bar;
pub mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};


/// State for the model-selector popup.
pub struct ModelSelectorState {
    pub models: Vec<String>,
    pub list_state: ListState,
}

impl ModelSelectorState {
    pub fn new(models: Vec<String>, current: &str) -> Self {
        let mut list_state = ListState::default();
        let idx = models
            .iter()
            .position(|m| m == current)
            .unwrap_or(0);
        list_state.select(Some(idx));
        Self { models, list_state }
    }

    pub fn next(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 1) % self.models.len()));
    }

    pub fn previous(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some(i.checked_sub(1).unwrap_or(self.models.len() - 1)));
    }

    pub fn selected_model(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.models.get(i))
            .map(String::as_str)
    }
}

/// Render the model selector as a centred popup overlay.
pub fn render_model_selector(frame: &mut Frame, state: &mut ModelSelectorState) {
    let area = centered_popup(40, (state.models.len() as u16) + 2, frame.area());

    // Clear the area behind the popup.
    frame.render_widget(Clear, area);

    let items: Vec<ListItem<'_>> = state
        .models
        .iter()
        .map(|m| {
            ListItem::new(Line::from(vec![Span::styled(
                format!("  {m}"),
                theme::text(),
            )]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Select Model ")
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border_active()),
        )
        .highlight_style(theme::selected())
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

/// Return a centred rectangle for popup dialogs.
fn centered_popup(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}
