use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AgentDispatchTarget;

use super::status_bar::{self, KeyHint};
use super::text_buffer::TextBufferState;
use super::theme;

pub struct AdditionalInstructionsState {
    pub target: AgentDispatchTarget,
    buffer: TextBufferState,
}

impl AdditionalInstructionsState {
    pub fn new(target: AgentDispatchTarget) -> Self {
        Self {
            target,
            buffer: TextBufferState::new(),
        }
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn handle_input(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.buffer.handle_input(code, modifiers);
    }
}

pub fn render(frame: &mut Frame, state: &AdditionalInstructionsState) {
    let area = frame.area();
    let popup = centered_popup(
        (area.width * 65 / 100).max(50),
        (area.height * 55 / 100).max(12),
        area,
    );

    frame.render_widget(Clear, popup);

    let [header_area, body_area, hint_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(popup);

    let header = Paragraph::new(Line::from(vec![Span::styled(
        " Additional instructions (optional)",
        theme::accent(),
    )]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme::border_active()),
    )
    .style(theme::text());
    frame.render_widget(header, header_area);

    let cursor = state.buffer.cursor();
    let mut lines: Vec<Line<'_>> = state
        .buffer
        .lines()
        .iter()
        .enumerate()
        .map(|(row, line)| {
            if row == cursor.0 {
                let col = cursor.1;
                let (before, after) = line.split_at(col.min(line.len()));
                let cursor_char = after.chars().next().unwrap_or(' ');
                let rest = if after.len() > cursor_char.len_utf8() {
                    &after[cursor_char.len_utf8()..]
                } else {
                    ""
                };
                Line::from(vec![
                    Span::styled(before, theme::text()),
                    Span::styled(cursor_char.to_string(), theme::selected()),
                    Span::styled(rest, theme::text()),
                ])
            } else {
                Line::styled(line, theme::text())
            }
        })
        .collect();

    if lines.is_empty() {
        lines.push(Line::styled("", theme::subtext()));
    }

    let body = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Instructions ")
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border_active()),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(body, body_area);

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

fn centered_popup(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}
