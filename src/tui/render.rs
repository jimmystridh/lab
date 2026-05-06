//! Full TUI layout rendering for the `lab` selector.

use crate::{
    entries::{has_date_prefix, Entry},
    fuzzy::MatchResult,
    tui::app::App,
    NO_COLORS,
};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::{sync::atomic::Ordering, time::SystemTime};

const TITLE: &str = " Lab Directory Selection";
const FOOTER_HINTS: &str =
    "↑/↓: Navigate  Enter: Select  ^R: Rename  ^G: Graduate  ^D: Delete  Esc: Cancel";
const CURSOR_HOME: &str = "\x1b[H";
const CLEAR_TO_END_SCREEN: &str = "\x1b[J";

/// Render a single frame for the current app state.
pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let colors_enabled = !NO_COLORS.load(Ordering::Relaxed);
    let lines = build_lines(app, area.width, area.height)
        .into_iter()
        .map(|line| line.into_ratatui_line(colors_enabled))
        .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(lines), area);
}

/// Render a stable newline-delimited snapshot for test mode.
pub fn render_snapshot(app: &App) -> String {
    let colors_enabled = !NO_COLORS.load(Ordering::Relaxed);
    render_snapshot_with_colors(
        app,
        app.terminal_size.width,
        app.terminal_size.height,
        colors_enabled,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Foreground {
    #[default]
    Default,
    Accent,
    Highlight,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Background {
    #[default]
    None,
    Selected,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SegmentStyleSpec {
    fg: Foreground,
    bg: Background,
    bold: bool,
    reversed: bool,
}

impl SegmentStyleSpec {
    const fn normal() -> Self {
        Self {
            fg: Foreground::Default,
            bg: Background::None,
            bold: false,
            reversed: false,
        }
    }

    const fn accent() -> Self {
        Self {
            fg: Foreground::Accent,
            bg: Background::None,
            bold: true,
            reversed: false,
        }
    }

    const fn highlight() -> Self {
        Self {
            fg: Foreground::Highlight,
            bg: Background::None,
            bold: true,
            reversed: false,
        }
    }

    const fn muted() -> Self {
        Self {
            fg: Foreground::Muted,
            bg: Background::None,
            bold: false,
            reversed: false,
        }
    }

    const fn cursor() -> Self {
        Self {
            fg: Foreground::Default,
            bg: Background::None,
            bold: false,
            reversed: true,
        }
    }

    const fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    const fn with_background(self, bg: Background) -> Self {
        Self { bg, ..self }
    }

    fn is_plain(self) -> bool {
        self == Self::normal()
    }

    fn to_ratatui(self, colors_enabled: bool) -> Style {
        if !colors_enabled {
            return Style::default();
        }

        let mut style = Style::default();
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.reversed {
            style = style.add_modifier(Modifier::REVERSED);
        }

        style = match self.fg {
            Foreground::Default => style,
            Foreground::Accent => style.fg(Color::Indexed(214)),
            Foreground::Highlight => style.fg(Color::Yellow),
            Foreground::Muted => style.fg(Color::Indexed(245)),
        };

        match self.bg {
            Background::None => style,
            Background::Selected => style.bg(Color::Indexed(238)),
            Background::Danger => style.bg(Color::Indexed(52)),
        }
    }

    fn ansi_prefix(self, colors_enabled: bool) -> String {
        if !colors_enabled || self.is_plain() {
            return String::new();
        }

        let mut codes = Vec::new();
        if self.bold {
            codes.push("1");
        }
        if self.reversed {
            codes.push("7");
        }

        match self.fg {
            Foreground::Default => {}
            Foreground::Accent => codes.push("38;5;214"),
            Foreground::Highlight => codes.push("33"),
            Foreground::Muted => codes.push("38;5;245"),
        }

        match self.bg {
            Background::None => {}
            Background::Selected => codes.push("48;5;238"),
            Background::Danger => codes.push("48;5;52"),
        }

        format!("\x1b[{}m", codes.join(";"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StyledSegment {
    text: String,
    style: SegmentStyleSpec,
}

impl StyledSegment {
    fn new(text: impl Into<String>, style: SegmentStyleSpec) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StyledLine {
    segments: Vec<StyledSegment>,
}

impl StyledLine {
    fn into_ratatui_line(self, colors_enabled: bool) -> Line<'static> {
        let spans = self
            .segments
            .into_iter()
            .map(|segment| Span::styled(segment.text, segment.style.to_ratatui(colors_enabled)))
            .collect::<Vec<_>>();

        Line::from(spans)
    }

    fn to_ansi_string(&self, colors_enabled: bool) -> String {
        let mut rendered = String::new();

        for segment in &self.segments {
            let prefix = segment.style.ansi_prefix(colors_enabled);
            if prefix.is_empty() {
                rendered.push_str(&segment.text);
            } else {
                rendered.push_str(&prefix);
                rendered.push_str(&segment.text);
                rendered.push_str("\x1b[0m");
            }
        }

        rendered
    }
}

fn render_snapshot_with_colors(app: &App, width: u16, height: u16, colors_enabled: bool) -> String {
    let rendered = build_lines(app, width, height)
        .into_iter()
        .map(|line| line.to_ansi_string(colors_enabled))
        .collect::<Vec<_>>();

    if rendered.is_empty() {
        String::new()
    } else {
        format!(
            "{CURSOR_HOME}{CLEAR_TO_END_SCREEN}{}\n",
            rendered.join("\n")
        )
    }
}

fn build_lines(app: &App, width: u16, height: u16) -> Vec<StyledLine> {
    if app.is_renaming() {
        return build_rename_dialog_lines(app, width, height);
    }

    if app.is_graduating() {
        return build_graduate_dialog_lines(app, width, height);
    }

    if app.is_confirming_delete() {
        return build_delete_confirmation_lines(app, width, height);
    }

    let body_rows = body_height(height);
    let mut lines = Vec::with_capacity(body_rows + 5);

    lines.push(build_title_line(app, width));
    lines.push(separator_line(width));
    lines.push(build_search_line(app, width));
    lines.extend(build_body_lines(app, width, body_rows));
    lines.push(separator_line(width));
    lines.push(build_footer_line(app, width));

    lines
}

fn build_title_line(app: &App, width: u16) -> StyledLine {
    compose_left_right(
        width,
        vec![
            StyledSegment::new("🏠", SegmentStyleSpec::normal()),
            StyledSegment::new(TITLE, SegmentStyleSpec::accent()),
        ],
        Some(StyledSegment::new(
            app.labs_path.display().to_string(),
            SegmentStyleSpec::muted(),
        )),
        Background::None,
    )
}

fn build_search_line(app: &App, width: u16) -> StyledLine {
    let mut segments = vec![StyledSegment::new("Search: ", SegmentStyleSpec::muted())];
    segments.extend(input_segments(&app.input, app.input_cursor_pos));

    fill_line(width, segments, Background::None)
}

fn build_body_lines(app: &App, width: u16, body_rows: usize) -> Vec<StyledLine> {
    let mut lines = Vec::with_capacity(body_rows);

    for row in 0..body_rows {
        let list_index = app.scroll_offset + row;
        let line = if list_index < app.filtered.len() {
            let result = &app.filtered[list_index];
            let entry = &app.entries[result.index];
            build_entry_line(app, entry, result, list_index == app.cursor_pos, width)
        } else if app.show_create_new() && list_index == app.filtered.len() {
            build_create_line(app, list_index == app.cursor_pos, width)
        } else {
            blank_line(width, Background::None)
        };

        lines.push(line);
    }

    lines
}

fn build_entry_line(
    app: &App,
    entry: &Entry,
    result: &MatchResult,
    selected: bool,
    width: u16,
) -> StyledLine {
    let marked = app.marks.contains(&result.index);
    let background = if marked {
        Background::Danger
    } else if selected {
        Background::Selected
    } else {
        Background::None
    };

    let line_fill = fill_style(background);
    let prefix_style = if selected {
        SegmentStyleSpec::highlight().with_background(background)
    } else {
        line_fill
    };

    let mut left = vec![
        StyledSegment::new(if selected { "→ " } else { "  " }, prefix_style),
        StyledSegment::new(
            if marked {
                "🗑️"
            } else if entry.is_symlink {
                "🔗"
            } else {
                "📁"
            },
            line_fill,
        ),
        StyledSegment::new(" ", line_fill),
    ];

    let name_segments = format_entry_name(&entry.name, &result.positions, background);
    let max_name_width = usize::from(width).saturating_sub(prefix_width() + 1);
    let name_width = text_width(&entry.name);

    let display_name = if name_width > max_name_width {
        truncated_name_segments(&name_segments, max_name_width, background)
    } else {
        name_segments
    };

    left.extend(display_name);

    if name_width > max_name_width {
        return fill_line(width, left, background);
    }

    let metadata = StyledSegment::new(
        format!("{}, {:.1}", format_relative_time(entry.mtime), result.score),
        SegmentStyleSpec::muted().with_background(background),
    );

    compose_left_right(width, left, Some(metadata), background)
}

fn build_footer_line(app: &App, width: u16) -> StyledLine {
    if app.is_delete_mode() {
        let text = format!(
            "DELETE MODE | {} marked | Ctrl-D: Toggle | Enter: Confirm | Esc: Cancel",
            app.marked_count()
        );
        centered_segments_line(
            width,
            vec![StyledSegment::new(
                text,
                SegmentStyleSpec::normal()
                    .bold()
                    .with_background(Background::Danger),
            )],
            Background::Danger,
        )
    } else {
        centered_line(width, FOOTER_HINTS, SegmentStyleSpec::muted())
    }
}

fn build_delete_confirmation_lines(app: &App, width: u16, height: u16) -> Vec<StyledLine> {
    let body_rows = dialog_body_height(height);
    let count = app.marked_count();
    let noun = if count == 1 {
        "directory"
    } else {
        "directories"
    };
    let marked_entries = app.marked_entries();
    let prompt_row = body_rows.saturating_sub(1);
    let visible_items = marked_entries.len().min(prompt_row);
    let mut lines = Vec::with_capacity(body_rows + 4);

    lines.push(centered_segments_line(
        width,
        vec![
            StyledSegment::new("🗑️", SegmentStyleSpec::normal()),
            StyledSegment::new(
                format!(" Delete {count} {noun}?"),
                SegmentStyleSpec::accent(),
            ),
        ],
        Background::None,
    ));
    lines.push(separator_line(width));

    for entry in marked_entries.iter().take(visible_items) {
        lines.push(build_delete_dialog_item_line(&entry.name, width));
    }
    while lines.len() < 2 + prompt_row {
        lines.push(blank_line(width, Background::None));
    }
    lines.push(build_delete_prompt_line(app, width));

    lines.push(separator_line(width));
    lines.push(centered_line(
        width,
        "Enter: Confirm  Esc: Cancel",
        SegmentStyleSpec::muted(),
    ));

    lines
}

fn build_rename_dialog_lines(app: &App, width: u16, height: u16) -> Vec<StyledLine> {
    let body_rows = dialog_body_height(height);
    let dialog = app.rename_dialog.as_ref();
    let current_name = dialog
        .map(|dialog| dialog.current_name.as_str())
        .unwrap_or_default();
    let input = dialog
        .map(|dialog| dialog.input.as_str())
        .unwrap_or_default();
    let cursor_pos = dialog.map(|dialog| dialog.cursor_pos).unwrap_or(0);
    let error = dialog.and_then(|dialog| dialog.error.as_deref());
    let mut body = vec![
        build_rename_current_line(current_name, width),
        blank_line(width, Background::None),
        blank_line(width, Background::None),
        build_rename_prompt_line(input, cursor_pos, width),
    ];

    if let Some(error) = error {
        body.push(blank_line(width, Background::None));
        body.push(centered_line(
            width,
            error,
            SegmentStyleSpec::normal().bold(),
        ));
    }

    body.truncate(body_rows);
    while body.len() < body_rows {
        body.push(blank_line(width, Background::None));
    }

    let mut lines = Vec::with_capacity(body_rows + 4);
    lines.push(centered_segments_line(
        width,
        vec![
            StyledSegment::new("✏️", SegmentStyleSpec::normal()),
            StyledSegment::new("  Rename directory", SegmentStyleSpec::accent()),
        ],
        Background::None,
    ));
    lines.push(separator_line(width));
    lines.extend(body);
    lines.push(separator_line(width));
    lines.push(centered_line(
        width,
        "Enter: Confirm  Esc: Cancel",
        SegmentStyleSpec::muted(),
    ));

    lines
}

fn build_graduate_dialog_lines(app: &App, width: u16, height: u16) -> Vec<StyledLine> {
    let body_rows = dialog_body_height(height);
    let dialog = app.graduate_dialog.as_ref();
    let current_name = dialog
        .map(|dialog| dialog.current_name.as_str())
        .unwrap_or_default();
    let input = dialog
        .map(|dialog| dialog.input.as_str())
        .unwrap_or_default();
    let cursor_pos = dialog.map(|dialog| dialog.cursor_pos).unwrap_or(0);
    let error = dialog.and_then(|dialog| dialog.error.as_deref());
    let destination_hint = dialog
        .map(|dialog| dialog.destination_hint.as_str())
        .unwrap_or("parent of $LAB_PATH");
    let destination_root = dialog
        .map(|dialog| dialog.destination_root.as_str())
        .unwrap_or_default();
    let mut body = vec![
        build_rename_current_line(current_name, width),
        blank_line(width, Background::None),
        build_graduate_hint_line(destination_hint, destination_root, width),
        build_graduate_prompt_line(input, cursor_pos, width),
    ];

    if let Some(error) = error {
        body.push(centered_line(
            width,
            "A symlink will be left in the labs directory",
            SegmentStyleSpec::muted(),
        ));
        body.push(centered_line(
            width,
            error,
            SegmentStyleSpec::normal().bold(),
        ));
    } else {
        body.push(blank_line(width, Background::None));
        body.push(centered_line(
            width,
            "A symlink will be left in the labs directory",
            SegmentStyleSpec::muted(),
        ));
    }

    body.truncate(body_rows);
    while body.len() < body_rows {
        body.push(blank_line(width, Background::None));
    }

    let mut lines = Vec::with_capacity(body_rows + 4);
    lines.push(centered_segments_line(
        width,
        vec![
            StyledSegment::new("🚀", SegmentStyleSpec::normal()),
            StyledSegment::new("  Graduate lab to project", SegmentStyleSpec::accent()),
        ],
        Background::None,
    ));
    lines.push(separator_line(width));
    lines.extend(body);
    lines.push(separator_line(width));
    lines.push(centered_line(
        width,
        "Enter: Confirm  Esc: Cancel",
        SegmentStyleSpec::muted(),
    ));

    lines
}

fn build_delete_dialog_item_line(name: &str, width: u16) -> StyledLine {
    fill_line(
        width,
        vec![
            StyledSegment::new("🗑️", fill_style(Background::Danger)),
            StyledSegment::new(" ", fill_style(Background::Danger)),
            StyledSegment::new(name, fill_style(Background::Danger)),
        ],
        Background::Danger,
    )
}

fn build_delete_prompt_line(app: &App, width: u16) -> StyledLine {
    let dialog = app.delete_confirmation.as_ref();
    let input = dialog
        .map(|dialog| dialog.input.as_str())
        .unwrap_or_default();
    let cursor_pos = dialog.map(|dialog| dialog.cursor_pos).unwrap_or(0);
    let mut segments = vec![StyledSegment::new(
        "Type YES to confirm: ",
        SegmentStyleSpec::muted(),
    )];
    segments.extend(input_segments(input, cursor_pos));
    centered_segments_line(width, segments, Background::None)
}

fn build_rename_current_line(current_name: &str, width: u16) -> StyledLine {
    fill_line(
        width,
        vec![
            StyledSegment::new("📁", SegmentStyleSpec::normal()),
            StyledSegment::new(" ", SegmentStyleSpec::normal()),
            StyledSegment::new(current_name, SegmentStyleSpec::normal()),
        ],
        Background::None,
    )
}

fn build_rename_prompt_line(input: &str, cursor_pos: usize, width: u16) -> StyledLine {
    let mut segments = vec![StyledSegment::new("New name: ", SegmentStyleSpec::muted())];
    segments.extend(input_segments(input, cursor_pos));
    centered_segments_line(width, segments, Background::None)
}

fn build_graduate_hint_line(
    destination_hint: &str,
    destination_root: &str,
    width: u16,
) -> StyledLine {
    centered_line(
        width,
        &format!("Destination ({destination_hint}: {destination_root})"),
        SegmentStyleSpec::muted(),
    )
}

fn build_graduate_prompt_line(input: &str, cursor_pos: usize, width: u16) -> StyledLine {
    let mut segments = vec![StyledSegment::new("Move to: ", SegmentStyleSpec::muted())];
    segments.extend(input_segments(input, cursor_pos));
    centered_segments_line(width, segments, Background::None)
}

fn build_create_line(app: &App, selected: bool, width: u16) -> StyledLine {
    let background = if selected {
        Background::Selected
    } else {
        Background::None
    };

    let line_fill = fill_style(background);
    let prefix_style = if selected {
        SegmentStyleSpec::highlight().with_background(background)
    } else {
        line_fill
    };
    let label = match app.create_new_name() {
        Some(name) => format!("Create new: {name}"),
        None => "Create new".to_string(),
    };

    fill_line(
        width,
        vec![
            StyledSegment::new(if selected { "→ " } else { "  " }, prefix_style),
            StyledSegment::new("📂", line_fill),
            StyledSegment::new(" ", line_fill),
            StyledSegment::new(label, line_fill),
        ],
        background,
    )
}

fn separator_line(width: u16) -> StyledLine {
    StyledLine {
        segments: vec![StyledSegment::new(
            "─".repeat(usize::from(width)),
            SegmentStyleSpec::muted(),
        )],
    }
}

fn centered_line(width: u16, text: &str, style: SegmentStyleSpec) -> StyledLine {
    centered_segments_line(
        width,
        vec![StyledSegment::new(text, style)],
        Background::None,
    )
}

fn centered_segments_line(
    width: u16,
    segments: Vec<StyledSegment>,
    background: Background,
) -> StyledLine {
    let width = usize::from(width);
    let content = truncate_segments(&segments, width);
    let content_width = segments_width(&content);
    let left_padding = width.saturating_sub(content_width) / 2;
    let right_padding = width.saturating_sub(content_width + left_padding);

    let mut segments = Vec::new();
    append_spaces(&mut segments, left_padding, fill_style(background));
    extend_segments(&mut segments, content);
    append_spaces(&mut segments, right_padding, fill_style(background));

    StyledLine { segments }
}

fn fill_line(width: u16, left: Vec<StyledSegment>, background: Background) -> StyledLine {
    compose_left_right(width, left, None, background)
}

fn compose_left_right(
    width: u16,
    left: Vec<StyledSegment>,
    right: Option<StyledSegment>,
    background: Background,
) -> StyledLine {
    let width = usize::from(width);
    let mut segments = truncate_segments(&left, width);
    let left_width = segments_width(&segments);

    if let Some(right) = right {
        let available_for_right = width.saturating_sub(left_width + 1);
        if available_for_right > 0 {
            let truncated_right = truncate_text_from_start(&right.text, available_for_right);
            let right_width = text_width(&truncated_right);
            let gap_width = width.saturating_sub(left_width + right_width);
            append_spaces(&mut segments, gap_width, fill_style(background));
            append_segment(
                &mut segments,
                truncated_right,
                right.style.with_background(background),
            );
            return StyledLine { segments };
        }
    }

    append_spaces(
        &mut segments,
        width.saturating_sub(left_width),
        fill_style(background),
    );
    StyledLine { segments }
}

fn blank_line(width: u16, background: Background) -> StyledLine {
    StyledLine {
        segments: vec![StyledSegment::new(
            " ".repeat(usize::from(width)),
            fill_style(background),
        )],
    }
}

fn format_entry_name(
    name: &str,
    positions: &[usize],
    background: Background,
) -> Vec<StyledSegment> {
    if has_date_prefix(name) && name.len() > 11 {
        let date_part = &name[..11];
        let name_part = &name[11..];
        let mut segments = vec![StyledSegment::new(
            date_part.to_string(),
            SegmentStyleSpec::muted().with_background(background),
        )];
        segments.extend(highlighted_name_segments(
            name_part, positions, 11, background,
        ));
        segments
    } else {
        highlighted_name_segments(name, positions, 0, background)
    }
}

fn highlighted_name_segments(
    text: &str,
    positions: &[usize],
    offset: usize,
    background: Background,
) -> Vec<StyledSegment> {
    if text.is_empty() {
        return Vec::new();
    }

    let base = SegmentStyleSpec::normal().with_background(background);
    let highlight = SegmentStyleSpec::highlight().with_background(background);
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut current_style = base;
    let mut initialized = false;
    let mut position_index = 0;

    for (index, ch) in text.chars().enumerate() {
        while position_index < positions.len() && positions[position_index] < index + offset {
            position_index += 1;
        }

        let style = if positions.get(position_index) == Some(&(index + offset)) {
            position_index += 1;
            highlight
        } else {
            base
        };

        if !initialized {
            current_style = style;
            initialized = true;
        }

        if style != current_style {
            append_segment(&mut segments, std::mem::take(&mut current), current_style);
            current_style = style;
        }

        current.push(ch);
    }

    append_segment(&mut segments, current, current_style);
    segments
}

fn input_segments(input: &str, cursor_pos: usize) -> Vec<StyledSegment> {
    let chars = input.chars().collect::<Vec<_>>();
    let cursor_pos = cursor_pos.min(chars.len());

    if chars.is_empty() {
        return vec![StyledSegment::new(" ", SegmentStyleSpec::cursor())];
    }

    let mut segments = Vec::new();

    if cursor_pos > 0 {
        append_segment(
            &mut segments,
            chars[..cursor_pos].iter().collect::<String>(),
            SegmentStyleSpec::normal(),
        );
    }

    let cursor_char = chars.get(cursor_pos).copied().unwrap_or(' ');
    append_segment(
        &mut segments,
        cursor_char.to_string(),
        SegmentStyleSpec::cursor(),
    );

    if cursor_pos < chars.len() {
        append_segment(
            &mut segments,
            chars[cursor_pos + 1..].iter().collect::<String>(),
            SegmentStyleSpec::normal(),
        );
    }

    segments
}

fn truncated_name_segments(
    segments: &[StyledSegment],
    max_width: usize,
    background: Background,
) -> Vec<StyledSegment> {
    match max_width {
        0 => Vec::new(),
        1 => vec![StyledSegment::new(
            "…",
            SegmentStyleSpec::normal().with_background(background),
        )],
        _ => {
            let mut truncated = truncate_segments(segments, max_width - 1);
            append_segment(
                &mut truncated,
                "…",
                SegmentStyleSpec::normal().with_background(background),
            );
            truncated
        }
    }
}

fn truncate_segments(segments: &[StyledSegment], max_width: usize) -> Vec<StyledSegment> {
    if max_width == 0 {
        return Vec::new();
    }

    let mut remaining = max_width;
    let mut truncated = Vec::new();

    for segment in segments {
        if remaining == 0 {
            break;
        }

        let text = take_prefix_by_width(&segment.text, remaining);
        let width = text_width(&text);
        append_segment(&mut truncated, text, segment.style);
        remaining = remaining.saturating_sub(width);
    }

    truncated
}

fn truncate_text_from_start(text: &str, max_width: usize) -> String {
    if text_width(text) <= max_width {
        return text.to_string();
    }

    take_suffix_by_width(text, max_width)
}

fn append_spaces(segments: &mut Vec<StyledSegment>, count: usize, style: SegmentStyleSpec) {
    if count > 0 {
        append_segment(segments, " ".repeat(count), style);
    }
}

fn extend_segments(target: &mut Vec<StyledSegment>, segments: Vec<StyledSegment>) {
    for segment in segments {
        append_segment(target, segment.text, segment.style);
    }
}

fn append_segment(
    segments: &mut Vec<StyledSegment>,
    text: impl Into<String>,
    style: SegmentStyleSpec,
) {
    let text = text.into();
    if text.is_empty() {
        return;
    }

    if let Some(last) = segments.last_mut() {
        if last.style == style {
            last.text.push_str(&text);
            return;
        }
    }

    segments.push(StyledSegment::new(text, style));
}

fn prefix_width() -> usize {
    text_width("→ 📁 ")
}

fn segments_width(segments: &[StyledSegment]) -> usize {
    segments
        .iter()
        .map(|segment| text_width(&segment.text))
        .sum()
}

fn text_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

fn take_prefix_by_width(text: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut truncated = String::new();

    for ch in text.chars() {
        let char_width = char_width(ch);
        if width + char_width > max_width {
            break;
        }
        truncated.push(ch);
        width += char_width;
    }

    truncated
}

fn take_suffix_by_width(text: &str, max_width: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut width = 0;
    let mut start = chars.len();

    while start > 0 {
        let next_width = char_width(chars[start - 1]);
        if width + next_width > max_width {
            break;
        }

        width += next_width;
        start -= 1;
    }

    chars[start..].iter().collect()
}

fn char_width(ch: char) -> usize {
    let code = ch as u32;
    if matches!(
        code,
        0x0300..=0x036F
            | 0x200B..=0x200D
            | 0xFE00..=0xFE0F
            | 0xE0100..=0xE01EF
    ) {
        0
    } else if matches!(
        code,
        0x1100..=0x115F
            | 0x2329..=0x232A
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
    ) {
        2
    } else {
        1
    }
}

fn fill_style(background: Background) -> SegmentStyleSpec {
    SegmentStyleSpec::normal().with_background(background)
}

fn body_height(height: u16) -> usize {
    usize::from(height.saturating_sub(5)).max(3)
}

fn dialog_body_height(height: u16) -> usize {
    usize::from(height.saturating_sub(4)).max(3)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{
        app::{Mode, TerminalSize},
        dialogs::GraduateDialog,
    };
    use std::{
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    fn make_entry(name: &str, is_symlink: bool, mtime: SystemTime) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            is_symlink,
            mtime,
            base_score: 0.0,
        }
    }

    fn make_app(entries: Vec<Entry>, width: u16, height: u16, input: Option<&str>) -> App {
        App::new(
            "/tmp/labs",
            entries,
            input,
            TerminalSize::new(width, height),
        )
    }

    fn snapshot(app: &App, colors_enabled: bool) -> String {
        render_snapshot_with_colors(
            app,
            app.terminal_size.width,
            app.terminal_size.height,
            colors_enabled,
        )
    }

    #[test]
    fn test_render_snapshot_includes_full_layout() {
        let mut app = make_app(
            vec![make_entry("2025-11-29-project", false, SystemTime::now())],
            60,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 5.0,
            positions: Vec::new(),
        }];

        let rendered = snapshot(&app, false);
        let lines = rendered.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 8);
        assert!(lines[0].contains("🏠"));
        assert!(lines[0].contains("Lab Directory Selection"));
        assert!(lines[0].ends_with("/tmp/labs"));
        assert_eq!(lines[1], "─".repeat(60));
        assert!(lines[2].starts_with("Search: "));
        assert!(lines[3].starts_with("→ 📁 "));
        assert_eq!(lines[6], "─".repeat(60));
        assert!(lines[7].contains("Enter: Select"));
    }

    #[test]
    fn test_render_snapshot_shows_reverse_video_cursor() {
        let app = make_app(Vec::new(), 40, 8, Some("lab"));
        let rendered = snapshot(&app, true);

        assert!(rendered.contains("lab\x1b[7m \x1b[0m"));
    }

    #[test]
    fn test_selected_line_uses_background_and_dimmed_date_prefix() {
        let mut app = make_app(
            vec![make_entry("2025-11-29-project", false, SystemTime::now())],
            80,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 4.2,
            positions: Vec::new(),
        }];

        let rendered = snapshot(&app, true);
        let selected_line = rendered.lines().nth(3).expect("selected line");

        assert!(selected_line.contains("\x1b[48;5;238m"));
        assert!(selected_line.contains("2025-11-29-"));
        assert!(selected_line.contains("38;5;245"));
        assert!(selected_line.contains("just now, 4.2"));
    }

    #[test]
    fn test_matched_characters_render_bold_yellow() {
        let mut app = make_app(
            vec![make_entry("2025-11-15-beta", false, SystemTime::now())],
            80,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 5.7,
            positions: vec![11, 12, 13],
        }];

        let rendered = snapshot(&app, true);
        let selected_line = rendered.lines().nth(3).expect("selected line");

        assert!(selected_line.contains("\x1b[1;33;48;5;238mbet\x1b[0m"));
    }

    #[test]
    fn test_symlink_entries_render_link_icon() {
        let mut app = make_app(
            vec![make_entry("linked-project", true, SystemTime::now())],
            60,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];

        let rendered = snapshot(&app, false);
        assert!(rendered.lines().nth(3).unwrap_or_default().contains("🔗"));
    }

    #[test]
    fn test_delete_mode_footer_uses_danger_background_and_mark_count() {
        let mut app = make_app(
            vec![make_entry("alpha", false, SystemTime::now())],
            80,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];
        app.marks.insert(0);

        let rendered = snapshot(&app, true);
        let footer = rendered.lines().nth(7).expect("footer line");

        assert!(footer
            .contains("DELETE MODE | 1 marked | Ctrl-D: Toggle | Enter: Confirm | Esc: Cancel"));
        assert!(footer.contains("\x1b[1;48;5;52m"));
    }

    #[test]
    fn test_marked_entry_uses_trash_icon_and_danger_background() {
        let mut app = make_app(
            vec![make_entry("2025-11-29-project", false, SystemTime::now())],
            80,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];
        app.marks.insert(0);

        let rendered = snapshot(&app, true);
        let line = rendered.lines().nth(3).expect("entry line");

        assert!(line.contains("🗑️"));
        assert!(line.contains("\x1b[48;5;52m"));
    }

    #[test]
    fn test_delete_confirmation_dialog_renders_marked_names_and_prompt() {
        let mut app = make_app(
            vec![
                make_entry("alpha", false, SystemTime::now()),
                make_entry("beta", false, SystemTime::now()),
            ],
            80,
            8,
            None,
        );
        app.filtered = vec![
            MatchResult {
                index: 0,
                score: 3.0,
                positions: Vec::new(),
            },
            MatchResult {
                index: 1,
                score: 2.0,
                positions: Vec::new(),
            },
        ];
        app.marks.insert(0);
        app.marks.insert(1);
        app.begin_delete_confirmation();
        if let Some(dialog) = app.delete_confirmation.as_mut() {
            dialog.input = "YES".to_string();
            dialog.cursor_pos = 3;
        }

        let rendered = snapshot(&app, true);
        let lines = rendered.lines().collect::<Vec<_>>();

        assert!(lines[0].contains("Delete 2 directories?"));
        assert!(lines.iter().any(|line| line.contains("🗑️ alpha")));
        assert!(lines.iter().any(|line| line.contains("🗑️ beta")));
        assert!(lines
            .iter()
            .any(|line| line.contains("Type YES to confirm: ")));
        assert!(rendered.contains("YES\x1b[7m \x1b[0m"));
    }

    #[test]
    fn test_rename_dialog_renders_title_current_name_and_prefilled_input() {
        let mut app = make_app(
            vec![make_entry("2025-11-29-project", false, SystemTime::now())],
            80,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];
        app.begin_rename();

        let rendered = snapshot(&app, true);
        let lines = rendered.lines().collect::<Vec<_>>();

        assert!(lines[0].contains("✏️"));
        assert!(lines[0].contains("Rename directory"));
        assert!(lines
            .iter()
            .any(|line| line.contains("📁 2025-11-29-project")));
        assert!(lines.iter().any(|line| line.contains("New name: ")));
        assert!(rendered.contains("2025-11-29-project\x1b[7m \x1b[0m"));
        assert!(lines
            .last()
            .is_some_and(|line| line.contains("Enter: Confirm  Esc: Cancel")));
    }

    #[test]
    fn test_rename_dialog_renders_validation_error() {
        let mut app = make_app(
            vec![make_entry("alpha", false, SystemTime::now())],
            80,
            10,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];
        app.begin_rename();
        if let Some(dialog) = app.rename_dialog.as_mut() {
            dialog.input.clear();
            dialog.cursor_pos = 0;
            dialog.set_error("Name cannot be empty");
        }

        let rendered = snapshot(&app, true);

        assert!(rendered.contains("Name cannot be empty"));
    }

    #[test]
    fn test_graduate_dialog_renders_title_destination_and_prefilled_input() {
        let mut app = make_app(
            vec![make_entry("2025-11-29-project", false, SystemTime::now())],
            90,
            10,
            None,
        );
        app.mode = Mode::Graduate;
        app.graduate_dialog = Some(GraduateDialog::new(
            "2025-11-29-project",
            "/tmp/projects/project",
            "$LAB_PROJECTS",
            "/tmp/projects",
        ));

        let rendered = snapshot(&app, true);
        let lines = rendered.lines().collect::<Vec<_>>();

        assert!(lines[0].contains("🚀"));
        assert!(lines[0].contains("Graduate lab to project"));
        assert!(lines
            .iter()
            .any(|line| line.contains("📁 2025-11-29-project")));
        assert!(lines
            .iter()
            .any(|line| line.contains("Destination ($LAB_PROJECTS: /tmp/projects)")));
        assert!(lines.iter().any(|line| line.contains("Move to: ")));
        assert!(rendered.contains("/tmp/projects/project\x1b[7m \x1b[0m"));
        assert!(lines
            .iter()
            .any(|line| line.contains("A symlink will be left in the labs directory")));
    }

    #[test]
    fn test_graduate_dialog_renders_validation_error() {
        let mut app = make_app(
            vec![make_entry("alpha", false, SystemTime::now())],
            90,
            10,
            None,
        );
        app.mode = Mode::Graduate;
        let mut dialog = GraduateDialog::new(
            "alpha",
            "/tmp/projects/alpha",
            "parent of $LAB_PATH",
            "/tmp/projects",
        );
        dialog.set_error("Destination cannot be empty");
        app.graduate_dialog = Some(dialog);

        let rendered = snapshot(&app, true);

        assert!(rendered.contains("Destination cannot be empty"));
    }

    #[test]
    fn test_metadata_truncates_from_left_when_space_is_limited() {
        let name = "2025-11-29-medium-length-project";
        let width = (prefix_width() + text_width(name) + 4) as u16;
        let mut app = make_app(
            vec![make_entry(name, false, SystemTime::now())],
            width,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];

        let rendered = snapshot(&app, false);
        let line = rendered.lines().nth(3).expect("entry line");

        assert!(line.contains(name));
        assert!(line.ends_with("3.0"));
        assert!(!line.contains("just now, 3.0"));
    }

    #[test]
    fn test_truncated_names_hide_metadata() {
        let name = "2025-11-29-extremely-long-project-name-that-needs-truncation";
        let mut app = make_app(
            vec![make_entry(name, false, SystemTime::now())],
            24,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];

        let rendered = snapshot(&app, false);
        let line = rendered.lines().nth(3).expect("entry line");

        assert!(line.contains("…"));
        assert!(!line.contains("3.0"));
    }

    #[test]
    fn test_truncated_selected_line_keeps_background_on_ellipsis() {
        let name = "2025-11-29-extremely-long-project-name-that-needs-truncation";
        let mut app = make_app(
            vec![make_entry(name, false, SystemTime::now())],
            24,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 3.0,
            positions: Vec::new(),
        }];

        let rendered = snapshot(&app, true);
        let line = rendered.lines().nth(3).expect("entry line");

        let bg_start = line
            .rfind("\x1b[48;5;238m")
            .expect("selected background segment");

        assert!(line[bg_start..].contains("…"));
    }

    #[test]
    fn test_render_snapshot_without_colors_omits_sgr_but_keeps_cursor_control() {
        let mut app = make_app(
            vec![make_entry("2025-11-29-project", false, SystemTime::now())],
            60,
            8,
            None,
        );
        app.filtered = vec![MatchResult {
            index: 0,
            score: 5.0,
            positions: Vec::new(),
        }];

        let rendered = snapshot(&app, false);

        assert!(rendered.starts_with("\x1b[H\x1b[J"));
        assert!(!rendered.contains("\x1b[0m"));
        assert!(!rendered.contains("\x1b[1m"));
        assert!(!rendered.contains("\x1b[7m"));
        assert!(!rendered.contains("\x1b[33m"));
        assert!(!rendered.contains("\x1b[38;5;"));
        assert!(!rendered.contains("\x1b[48;5;"));
    }

    #[test]
    fn test_format_relative_time_thresholds() {
        let now = SystemTime::now();

        assert_eq!(
            format_relative_time(now - Duration::from_secs(59)),
            "just now"
        );
        assert_eq!(
            format_relative_time(now - Duration::from_secs(60)),
            "1m ago"
        );
        assert_eq!(
            format_relative_time(now - Duration::from_secs(3_600)),
            "1h ago"
        );
        assert_eq!(
            format_relative_time(now - Duration::from_secs(86_400)),
            "1d ago"
        );
        assert_eq!(
            format_relative_time(now - Duration::from_secs(604_800)),
            "1w ago"
        );
    }
}
