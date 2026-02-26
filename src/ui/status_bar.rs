use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme;

/// A key–description pair shown in the status bar.
pub struct KeyHint {
    pub key: &'static str,
    pub desc: &'static str,
}

/// Render a status bar at the bottom of `area` with the given hints.
pub fn render(frame: &mut Frame, area: Rect, hints: &[KeyHint]) {
    let mut spans = Vec::new();

    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme::status_bar()));
        }
        spans.push(Span::styled(
            format!("[{}]", hint.key),
            theme::key_hint(),
        ));
        spans.push(Span::styled(format!(" {}", hint.desc), theme::status_bar()));
    }

    let bar = Paragraph::new(Line::from(spans)).style(theme::status_bar());
    frame.render_widget(bar, area);
}
