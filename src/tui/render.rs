//! Basic ratatui rendering for the `lab` selector.

use crate::{entries::Entry, tui::app::App, NO_COLORS};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::{sync::atomic::Ordering, time::SystemTime};

/// Render a single frame for the current app state.
pub fn render(frame: &mut Frame<'_>, app: &App) {
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(frame.area());

    render_header(frame, areas[0], app);
    render_body(frame, areas[1], app);
    render_footer(frame, areas[2]);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let muted = muted_style();
    let lines = vec![
        Line::from(format!("🏠 {}", app.labs_path.display())),
        Line::from(Span::styled(separator(area.width), muted)),
        Line::from(vec![
            Span::styled("Search: ", muted),
            Span::raw(app.input.as_str()),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let visible_rows = usize::from(area.height);
    let mut lines = Vec::new();

    let end = (app.scroll_offset + visible_rows).min(app.total_items());
    for list_index in app.scroll_offset..end {
        if list_index < app.filtered.len() {
            let result = &app.filtered[list_index];
            let entry = &app.entries[result.index];
            lines.push(render_entry_line(
                entry,
                result.score,
                list_index == app.cursor_pos,
                area,
            ));
        } else if app.show_create_new() {
            let new_name = app.create_new_name().unwrap_or_default();
            lines.push(render_virtual_line(
                list_index == app.cursor_pos,
                "📂",
                format!("[new] {new_name}"),
            ));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    let muted = muted_style();
    let lines = vec![
        Line::from(Span::styled(separator(area.width), muted)),
        Line::from(Span::styled(
            "Navigate: ↑/↓  Select: Enter  ^R: Rename  ^G: Graduate  ^D: Delete  Esc: Cancel",
            muted,
        )),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_entry_line(entry: &Entry, score: f64, selected: bool, area: Rect) -> Line<'static> {
    let indicator = if selected { "→" } else { " " };
    let icon = if entry.is_symlink { "🔗" } else { "📁" };
    let metadata = format!("{}, {:.1}", format_relative_time(entry.mtime), score);
    let prefix_width = 6;
    let suffix_width = metadata.chars().count() + 2;
    let available = usize::from(area.width).saturating_sub(prefix_width + suffix_width);
    let display_name = truncate_name(&entry.name, available);

    if NO_COLORS.load(Ordering::Relaxed) {
        return Line::from(format!("{indicator} {icon} {display_name}  {metadata}"));
    }

    let selected_style = selected_style(selected);
    Line::from(vec![
        Span::styled(indicator.to_string(), selected_style),
        Span::raw(" "),
        Span::styled(icon.to_string(), selected_style),
        Span::raw(" "),
        Span::styled(display_name, selected_style),
        Span::raw("  "),
        Span::styled(metadata, muted_style()),
    ])
}

fn render_virtual_line(selected: bool, icon: &str, text: String) -> Line<'static> {
    if NO_COLORS.load(Ordering::Relaxed) {
        return Line::from(format!(
            "{} {} {}",
            if selected { "→" } else { " " },
            icon,
            text
        ));
    }

    let style = selected_style(selected);
    Line::from(vec![
        Span::styled(if selected { "→" } else { " " }, style),
        Span::raw(" "),
        Span::styled(icon.to_string(), style),
        Span::raw(" "),
        Span::styled(text, style),
    ])
}

fn separator(width: u16) -> String {
    "─".repeat(usize::from(width))
}

fn truncate_name(name: &str, max_width: usize) -> String {
    let char_count = name.chars().count();
    if max_width == 0 {
        return String::new();
    }
    if char_count <= max_width {
        return name.to_string();
    }
    if max_width == 1 {
        return "…".to_string();
    }

    let truncated: String = name.chars().take(max_width - 1).collect();
    format!("{truncated}…")
}

fn format_relative_time(mtime: SystemTime) -> String {
    let elapsed = SystemTime::now().duration_since(mtime).unwrap_or_default();
    let seconds = elapsed.as_secs();

    if seconds < 60 {
        "just now".to_string()
    } else if seconds < 3_600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3_600)
    } else if seconds < 604_800 {
        format!("{}d ago", seconds / 86_400)
    } else {
        format!("{}w ago", seconds / 604_800)
    }
}

fn muted_style() -> Style {
    if NO_COLORS.load(Ordering::Relaxed) {
        Style::default()
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

fn selected_style(selected: bool) -> Style {
    if NO_COLORS.load(Ordering::Relaxed) || !selected {
        Style::default()
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}
