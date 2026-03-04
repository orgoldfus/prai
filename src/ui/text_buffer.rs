use crossterm::event::{KeyCode, KeyModifiers};

pub struct TextBufferState {
    lines: Vec<String>,
    cursor: (usize, usize),
}

impl TextBufferState {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

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

/// Render the buffer contents with a block-cursor on the active line.
pub fn render_lines<'a>(buffer: &'a TextBufferState) -> Vec<ratatui::text::Line<'a>> {
    use ratatui::text::{Line, Span};

    let cursor = buffer.cursor();
    let mut lines: Vec<Line<'a>> = buffer
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
                    Span::styled(before.to_owned(), super::theme::text()),
                    Span::styled(cursor_char.to_string(), super::theme::selected()),
                    Span::styled(rest.to_owned(), super::theme::text()),
                ])
            } else {
                Line::styled(line.to_owned(), super::theme::text())
            }
        })
        .collect();

    if lines.is_empty() {
        lines.push(Line::styled("".to_owned(), super::theme::subtext()));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::TextBufferState;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn inserts_text_and_newline() {
        let mut state = TextBufferState::new();
        state.handle_input(KeyCode::Char('h'), KeyModifiers::NONE);
        state.handle_input(KeyCode::Char('i'), KeyModifiers::NONE);
        state.handle_input(KeyCode::Enter, KeyModifiers::NONE);
        state.handle_input(KeyCode::Char('x'), KeyModifiers::NONE);

        assert_eq!(state.lines(), &["hi".to_owned(), "x".to_owned()]);
        assert_eq!(state.text(), "hi\nx");
    }

    #[test]
    fn backspace_joins_lines_at_row_start() {
        let mut state = TextBufferState::new();
        state.handle_input(KeyCode::Char('a'), KeyModifiers::NONE);
        state.handle_input(KeyCode::Enter, KeyModifiers::NONE);
        state.handle_input(KeyCode::Char('b'), KeyModifiers::NONE);
        state.handle_input(KeyCode::Backspace, KeyModifiers::NONE);
        state.handle_input(KeyCode::Backspace, KeyModifiers::NONE);

        assert_eq!(state.lines(), &["a".to_owned()]);
        assert_eq!(state.cursor(), (0, 1));
    }

    #[test]
    fn cursor_navigation_handles_utf8() {
        let mut state = TextBufferState::new();
        state.handle_input(KeyCode::Char('a'), KeyModifiers::NONE);
        state.handle_input(KeyCode::Char('é'), KeyModifiers::NONE);
        state.handle_input(KeyCode::Left, KeyModifiers::NONE);
        state.handle_input(KeyCode::Right, KeyModifiers::NONE);
        state.handle_input(KeyCode::Enter, KeyModifiers::NONE);
        state.handle_input(KeyCode::Up, KeyModifiers::NONE);
        state.handle_input(KeyCode::Down, KeyModifiers::NONE);

        assert_eq!(state.cursor(), (1, 0));
    }
}
