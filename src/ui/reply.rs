use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::status_bar::{self, KeyHint};
use super::text_buffer::{self, TextBufferState};
use super::theme;

/// State for the reply text-input popup.
pub struct ReplyState {
    pub thread_id: String,
    #[allow(dead_code)]
    pub pr_number: u64,
    pub path: String,
    buffer: TextBufferState,
}

impl ReplyState {
    pub fn new(thread_id: String, pr_number: u64, path: String) -> Self {
        Self {
            thread_id,
            pr_number,
            path,
            buffer: TextBufferState::new(),
        }
    }

    /// Get the full reply text.
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    /// Handle a key input event.
    pub fn handle_input(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.buffer.handle_input(code, modifiers);
    }
}

/// Render the reply popup as a centred overlay.
pub fn render(frame: &mut Frame, state: &ReplyState) {
    let area = frame.area();
    let popup = super::centered_popup(
        (area.width * 60 / 100).max(40),
        (area.height * 50 / 100).max(10),
        area,
    );

    frame.render_widget(Clear, popup);

    let [header_area, body_area, hint_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(popup);

    // Header: file path.
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" Reply to: ", theme::accent()),
        Span::styled(&state.path, theme::text()),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme::border_active()),
    )
    .style(theme::text());
    frame.render_widget(header, header_area);

    // Body: text area with cursor.
    let lines = text_buffer::render_lines(&state.buffer);

    let body = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Message ")
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border_active()),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(body, body_area);

    // Hints.
    status_bar::render(
        frame,
        hint_area,
        &[
            KeyHint {
                key: "^s",
                desc: "Submit",
            },
            KeyHint {
                key: "Esc",
                desc: "Cancel",
            },
        ],
    );
}

