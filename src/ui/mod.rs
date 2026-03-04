pub mod additional_instructions;
pub mod agent_timeline;
pub mod comment_detail;
pub mod comment_list;
pub mod pr_list;
pub mod reply;
pub mod splash;
pub mod status_bar;
pub mod text_buffer;
pub mod theme;

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

/// State for the model-selector popup.
pub struct ModelSelectorState {
    pub models: Vec<String>,
    pub filter: String,
    pub list_state: ListState,
}

impl ModelSelectorState {
    pub fn new(models: Vec<String>, current: &str) -> Self {
        let mut list_state = ListState::default();
        let idx = models.iter().position(|m| m == current).unwrap_or(0);
        list_state.select(Some(idx));
        Self {
            models,
            filter: String::new(),
            list_state,
        }
    }

    /// Indices into `models` that match the current filter.
    fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.is_empty() {
            return (0..self.models.len()).collect();
        }
        let query = self.filter.to_lowercase();
        self.models
            .iter()
            .enumerate()
            .filter(|(_, m)| m.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn next(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            self.list_state.select(None);
            return;
        }
        let current = self.list_state.selected();
        let pos = current.and_then(|sel| indices.iter().position(|&i| i == sel));
        let next = match pos {
            Some(p) => indices[(p + 1) % indices.len()],
            None => indices[0],
        };
        self.list_state.select(Some(next));
    }

    pub fn previous(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            self.list_state.select(None);
            return;
        }
        let current = self.list_state.selected();
        let pos = current.and_then(|sel| indices.iter().position(|&i| i == sel));
        let prev = match pos {
            Some(p) => indices[p.checked_sub(1).unwrap_or(indices.len() - 1)],
            None => *indices.last().unwrap(),
        };
        self.list_state.select(Some(prev));
    }

    pub fn selected_model(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.models.get(i))
            .map(String::as_str)
    }

    pub fn push_filter_char(&mut self, c: char) {
        self.filter.push(c);
        self.snap_selection();
    }

    pub fn pop_filter_char(&mut self) {
        self.filter.pop();
        self.snap_selection();
    }

    /// Ensure the selection points to a visible item after the filter changes.
    fn snap_selection(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            self.list_state.select(None);
            return;
        }
        let still_visible = self
            .list_state
            .selected()
            .is_some_and(|sel| indices.contains(&sel));
        if !still_visible {
            self.list_state.select(Some(indices[0]));
        }
    }
}

/// Render the model selector as a centred popup overlay.
pub fn render_model_selector(frame: &mut Frame, state: &mut ModelSelectorState) {
    let filtered: Vec<usize> = state.filtered_indices();
    let visible_count = filtered.len() as u16;
    // +2 for list border, +2 for search input row + its border
    let height = visible_count.min(20) + 4;
    let area = centered_popup(50, height, frame.area());

    frame.render_widget(Clear, area);

    let [search_area, list_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);

    // Search input.
    let cursor_char = if state.filter.is_empty() { "_" } else { "▏" };
    let search_line = Line::from(vec![
        Span::styled(" 🔍 ", theme::accent()),
        Span::styled(&state.filter, theme::text()),
        Span::styled(cursor_char, theme::subtext()),
    ]);
    let search = Paragraph::new(search_line).block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::TOP)
            .border_style(theme::border_active()),
    );
    frame.render_widget(search, search_area);

    // Filtered model list.
    let items: Vec<ListItem<'_>> = filtered
        .iter()
        .map(|&i| {
            ListItem::new(Line::from(vec![Span::styled(
                format!("  {}", &state.models[i]),
                theme::text(),
            )]))
        })
        .collect();

    // Map the real selection index to the position within the filtered list.
    let mut view_state = ListState::default();
    if let Some(sel) = state.list_state.selected() {
        let pos = filtered.iter().position(|&i| i == sel);
        view_state.select(pos);
    }

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

    frame.render_stateful_widget(list, list_area, &mut view_state);
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
