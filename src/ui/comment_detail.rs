use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::comment_list::CommentEntry;
use super::status_bar::{self, KeyHint};
use super::theme;

/// Render a full-screen detail view for a single comment.
pub fn render(frame: &mut Frame, entry: &CommentEntry) {
    let area = frame.area();

    // Count actual lines needed for the comment body.
    let mut body_line_count: u16 = 2; // root "@author:" + one blank
    body_line_count += entry.body.lines().count() as u16;
    for reply in &entry.replies {
        body_line_count += 2; // separator + "@author:"
        body_line_count += reply.body.lines().count() as u16;
    }
    body_line_count += 2; // block borders

    let max_body = (area.height.saturating_sub(4 + 1) * 60) / 100; // at most 60% of usable space
    let body_height = body_line_count.clamp(4, max_body);

    let [header_area, body_area, diff_area, status_area] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(body_height),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(area);

    // ── Header ────────────────────────────────────────────────────────
    let location = if let Some(line) = entry.line {
        format!("{}:{line}", entry.path)
    } else {
        entry.path.clone()
    };

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" 🙏 PRAI", theme::accent()),
            Span::styled("  │  ", theme::border()),
            Span::styled("Comment Detail", theme::text()),
        ]),
        Line::from(vec![
            Span::styled("   📄 ", theme::accent()),
            Span::styled(&location, theme::text()),
            Span::styled("  by ", theme::subtext()),
            Span::styled(format!("@{}", entry.author), theme::author()),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme::border()),
    )
    .style(theme::text());

    frame.render_widget(header, header_area);

    // ── Comment body + thread replies ──────────────────────────────────
    let mut body_lines: Vec<Line<'_>> = Vec::new();

    // Root comment.
    body_lines.push(Line::from(vec![
        Span::styled(format!("@{}", entry.author), theme::author()),
        Span::styled(":", theme::text()),
    ]));
    for l in entry.body.lines() {
        body_lines.push(Line::styled(format!("  {l}"), theme::text()));
    }

    // Thread replies.
    for reply in &entry.replies {
        body_lines.push(Line::styled("  ─────", theme::border()));
        body_lines.push(Line::from(vec![
            Span::styled(format!("  @{}", reply.author), theme::author()),
            Span::styled(":", theme::text()),
        ]));
        for l in reply.body.lines() {
            body_lines.push(Line::styled(format!("    {l}"), theme::text()));
        }
    }

    let title = if entry.replies.is_empty() {
        " Comment ".to_owned()
    } else {
        format!(" Thread ({} replies) ", entry.replies.len())
    };

    let body = Paragraph::new(Text::from(body_lines))
        .block(
            Block::default()
                .title(title)
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border()),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(body, body_area);

    // ── Diff hunk ─────────────────────────────────────────────────────
    render_diff(frame, diff_area, &entry.diff_hunk);

    // ── Status bar ────────────────────────────────────────────────────
    status_bar::render(
        frame,
        status_area,
        &[
            KeyHint {
                key: "a",
                desc: "Send this to agent",
            },
            KeyHint {
                key: "r",
                desc: "Reply",
            },
            KeyHint {
                key: "o",
                desc: "Browser",
            },
            KeyHint {
                key: "t",
                desc: "👍",
            },
            KeyHint {
                key: "q",
                desc: "Back",
            },
        ],
    );
}

fn render_diff(frame: &mut Frame, area: Rect, diff_hunk: &str) {
    let lines: Vec<Line<'_>> = if diff_hunk.is_empty() {
        vec![Line::styled(
            "  (no diff context available)",
            theme::subtext(),
        )]
    } else {
        diff_hunk
            .lines()
            .map(|l| {
                let style = if l.starts_with('+') {
                    theme::diff_add()
                } else if l.starts_with('-') {
                    theme::diff_del()
                } else if l.starts_with("@@") {
                    theme::subtext()
                } else {
                    theme::text()
                };
                Line::styled(format!("  {l}"), style)
            })
            .collect()
    };

    let diff = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title(" Diff Context ")
                .title_style(theme::accent())
                .borders(Borders::ALL)
                .border_style(theme::border()),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(diff, area);
}
