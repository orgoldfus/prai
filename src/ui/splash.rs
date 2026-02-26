use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::theme;

const LOGO: &str = r#"
        🙏

 ██████╗ ██████╗  █████╗ ██╗
 ██╔══██╗██╔══██╗██╔══██╗██║
 ██████╔╝██████╔╝███████║██║
 ██╔═══╝ ██╔══██╗██╔══██║██║
 ██║     ██║  ██║██║  ██║██║
 ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝

 AI-Powered Code Review Assistant
"#;

/// Render the splash screen centred in the terminal.
pub fn render(frame: &mut Frame) {
    let area = frame.area();

    // Clear the entire screen with the base colour.
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(theme::text()),
        area,
    );

    let lines: Vec<Line<'_>> = LOGO
        .lines()
        .map(|l| Line::from(l).style(theme::accent()))
        .collect();

    let logo_height = lines.len() as u16;
    let text = Text::from(lines);

    // Centre vertically.
    let vertical = Layout::vertical([Constraint::Length(logo_height)])
        .flex(Flex::Center)
        .split(area);

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default());

    frame.render_widget(paragraph, centered_rect(60, logo_height, vertical[0]));
}

/// Return a centred rectangle of the given width percentage and fixed height.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .split(area);

    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(horizontal[0]);

    vertical[0]
}
