use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::status_bar::{self, KeyHint};
use super::theme;

/// State for the reply text-input popup.
pub struct ReplyState {
    pub thread_id: String,
    #[allow(dead_code)]
    pub pr_number: u64,
    pub path: String,
    /// Lines of text in the reply buffer.
    lines: Vec<String>,
    /// Cursor position: (line, column).
    cursor: (usize, usize),
}

impl ReplyState {
    pub fn new(thread_id: String, pr_number: u64, path: String) -> Self {
        Self {
            thread_id,
            pr_number,
            path,
            lines: vec![String::new()],
            cursor: (0, 0),
        }
    }

    /// Get the full reply text.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Handle a key input event.
    pub fn handle_input(&mut self, code: KeyCode, _modifiers: KeyModifiers) {
        match code {
            KeyCode::Char(c) => {
                let (row, col) = self.cursor;
                self.lines[row].insert(col, c);
                self.cursor.1 += c.len_utf8();
            }
            KeyCode::Enter => {
                let (row, col) = self.cursor;
                let tail = self.lines[row][col..].to_owned();
                self.lines[row].truncate(col);
                self.lines.insert(row + 1, tail);
                self.cursor = (row + 1, 0);
            }
            KeyCode::Backspace => {
                let (row, col) = self.cursor;
                if col > 0 {
                    // Find the previous character boundary.
                    let prev = self.lines[row][..col]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.lines[row].drain(prev..col);
                    self.cursor.1 = prev;
                } else if row > 0 {
                    let line = self.lines.remove(row);
                    let new_col = self.lines[row - 1].len();
                    self.lines[row - 1].push_str(&line);
                    self.cursor = (row - 1, new_col);
                }
            }
            KeyCode::Left => {
                if self.cursor.1 > 0 {
                    let prev = self.lines[self.cursor.0][..self.cursor.1]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.cursor.1 = prev;
                } else if self.cursor.0 > 0 {
                    self.cursor.0 -= 1;
                    self.cursor.1 = self.lines[self.cursor.0].len();
                }
            }
            KeyCode::Right => {
                let line = &self.lines[self.cursor.0];
                if self.cursor.1 < line.len() {
                    let next = line[self.cursor.1..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor.1 + i)
                        .unwrap_or(line.len());
                    self.cursor.1 = next;
                } else if self.cursor.0 < self.lines.len() - 1 {
                    self.cursor.0 += 1;
                    self.cursor.1 = 0;
                }
            }
            KeyCode::Up => {
                if self.cursor.0 > 0 {
                    self.cursor.0 -= 1;
                    self.cursor.1 = self.cursor.1.min(self.lines[self.cursor.0].len());
                }
            }
            KeyCode::Down => {
                if self.cursor.0 < self.lines.len() - 1 {
                    self.cursor.0 += 1;
                    self.cursor.1 = self.cursor.1.min(self.lines[self.cursor.0].len());
                }
            }
            _ => {}
        }
    }
}

/// Render the reply popup as a centred overlay.
pub fn render(frame: &mut Frame, state: &ReplyState) {
    let area = frame.area();
    let popup = centered_popup(
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
    let mut lines: Vec<Line<'_>> = state
        .lines
        .iter()
        .enumerate()
        .map(|(row, line)| {
            if row == state.cursor.0 {
                // Show cursor as a block character.
                let col = state.cursor.1;
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

fn centered_popup(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}
