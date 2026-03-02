use ratatui::style::{Color, Modifier, Style};

// ── Catppuccin Mocha palette ──────────────────────────────────────────────

pub const BASE: Color = Color::Rgb(30, 30, 46); // #1e1e2e
#[allow(dead_code)]
pub const MANTLE: Color = Color::Rgb(24, 24, 37); // #181825
pub const SURFACE0: Color = Color::Rgb(49, 50, 68); // #313244
#[allow(dead_code)]
pub const SURFACE1: Color = Color::Rgb(69, 71, 90); // #45475a
pub const OVERLAY0: Color = Color::Rgb(108, 112, 134); // #6c7086
pub const TEXT: Color = Color::Rgb(205, 214, 244); // #cdd6f4
pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200); // #a6adc8
pub const MAUVE: Color = Color::Rgb(203, 166, 247); // #cba6f7
pub const PEACH: Color = Color::Rgb(250, 179, 135); // #fab387
pub const GREEN: Color = Color::Rgb(166, 227, 161); // #a6e3a1
pub const RED: Color = Color::Rgb(243, 139, 168); // #f38ba8
pub const BLUE: Color = Color::Rgb(137, 180, 250); // #89b4fa
pub const YELLOW: Color = Color::Rgb(249, 226, 175); // #f9e2af
#[allow(dead_code)]
pub const LAVENDER: Color = Color::Rgb(180, 190, 254); // #b4befe

// ── Semantic styles ───────────────────────────────────────────────────────

/// Default text style.
pub fn text() -> Style {
    Style::default().fg(TEXT).bg(BASE)
}

/// Dimmed / secondary text.
pub fn subtext() -> Style {
    Style::default().fg(SUBTEXT0)
}

/// Accent style for titles, the splash logo, etc.
pub fn accent() -> Style {
    Style::default().fg(PEACH).add_modifier(Modifier::BOLD)
}

/// Highlighted / selected item in a list.
pub fn selected() -> Style {
    Style::default()
        .fg(MAUVE)
        .bg(SURFACE0)
        .add_modifier(Modifier::BOLD)
}

/// Border around panels.
pub fn border() -> Style {
    Style::default().fg(OVERLAY0)
}

/// Active / focused border.
pub fn border_active() -> Style {
    Style::default().fg(MAUVE)
}

/// Diff addition line.
pub fn diff_add() -> Style {
    Style::default().fg(GREEN)
}

/// Diff removal line.
pub fn diff_del() -> Style {
    Style::default().fg(RED)
}

/// Author name.
pub fn author() -> Style {
    Style::default().fg(BLUE)
}

/// Status bar background.
pub fn status_bar() -> Style {
    Style::default().fg(TEXT).bg(SURFACE0)
}

/// Keyboard shortcut key in the status bar.
pub fn key_hint() -> Style {
    Style::default()
        .fg(PEACH)
        .bg(SURFACE0)
        .add_modifier(Modifier::BOLD)
}

/// Success message.
pub fn success() -> Style {
    Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
}

/// Error message.
pub fn error() -> Style {
    Style::default().fg(RED).add_modifier(Modifier::BOLD)
}

/// Style for the checkbox when a comment is selected.
pub fn checkbox() -> Style {
    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
}
