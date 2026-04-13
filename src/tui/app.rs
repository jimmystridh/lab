//! Application state for the `lab` TUI.
//!
//! This module owns the selector state shared between rendering and input
//! handling: loaded entries, current filter results, input buffer, selection,
//! scroll position, mode, delete marks, and terminal size.

use crate::{
    entries::Entry,
    fuzzy::{Fuzzy, MatchResult},
};
use chrono::Local;
use crossterm::terminal;
use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
};

/// Terminal dimensions used by the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    /// Terminal width in columns.
    pub width: u16,
    /// Terminal height in rows.
    pub height: u16,
}

impl TerminalSize {
    /// Create a new terminal size, clamping both dimensions to at least 1.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }

    /// Detect terminal size, honoring LAB_WIDTH/LAB_HEIGHT overrides.
    pub fn detect() -> Self {
        let (detected_width, detected_height) = terminal::size().unwrap_or((80, 24));
        Self::new(
            resolve_dimension(env::var("LAB_WIDTH").ok().as_deref(), detected_width, 80),
            resolve_dimension(env::var("LAB_HEIGHT").ok().as_deref(), detected_height, 24),
        )
    }
}

/// Selector mode.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal entry-selection mode.
    Normal,
}

/// Selection outcome derived from the current cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Existing entry selected.
    Existing(PathBuf),
    /// Virtual "create new" row selected.
    Create(PathBuf),
}

/// Full TUI application state.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct App {
    /// Labs root directory.
    pub labs_path: PathBuf,
    /// All loaded entries.
    pub entries: Vec<Entry>,
    /// Filtered entries currently visible to the selector.
    pub filtered: Vec<MatchResult>,
    /// Search input buffer.
    pub input: String,
    /// Cursor position within the input buffer.
    pub input_cursor_pos: usize,
    /// Selected row in the filtered results / create-new list.
    pub cursor_pos: usize,
    /// Scroll offset for long result sets.
    pub scroll_offset: usize,
    /// Current selector mode.
    pub mode: Mode,
    /// Marked entry indices (used by later delete mode features).
    pub marks: HashSet<usize>,
    /// Current terminal dimensions.
    pub terminal_size: TerminalSize,
}

impl App {
    /// Build a new app from loaded entries and optional initial input.
    pub fn new<P>(
        labs_path: P,
        entries: Vec<Entry>,
        initial_input: Option<&str>,
        size: TerminalSize,
    ) -> Self
    where
        P: AsRef<Path>,
    {
        let input = initial_input.unwrap_or_default().to_string();
        let input_cursor_pos = input.chars().count();
        let mut app = Self {
            labs_path: labs_path.as_ref().to_path_buf(),
            entries,
            filtered: Vec::new(),
            input,
            input_cursor_pos,
            cursor_pos: 0,
            scroll_offset: 0,
            mode: Mode::Normal,
            marks: HashSet::new(),
            terminal_size: size,
        };
        app.refresh_filtered();
        app
    }

    /// Update the known terminal size and recompute visible results.
    pub fn set_terminal_size(&mut self, size: TerminalSize) {
        self.terminal_size = size;
        self.refresh_filtered();
    }

    /// Maximum number of real entries shown at once.
    pub fn visible_result_limit(&self) -> usize {
        usize::from(self.terminal_size.height.saturating_sub(5)).max(3)
    }

    /// Whether the virtual "create new" row should be shown.
    pub fn show_create_new(&self) -> bool {
        !self.input.is_empty()
    }

    /// Total selectable rows, including the virtual "create new" row when present.
    pub fn total_items(&self) -> usize {
        self.filtered.len() + usize::from(self.show_create_new())
    }

    /// Return the normalized create-new name with today's date prefix.
    pub fn create_new_name(&self) -> Option<String> {
        if self.input.is_empty() {
            return None;
        }

        let date_prefix = Local::now().format("%Y-%m-%d");
        Some(format!("{}-{}", date_prefix, self.input.replace(' ', "-")))
    }

    /// Resolve the current selection to an existing path or create-new path.
    pub fn current_selection(&self) -> Option<Selection> {
        if self.cursor_pos < self.filtered.len() {
            let index = self.filtered[self.cursor_pos].index;
            return Some(Selection::Existing(self.entries[index].path.clone()));
        }

        self.create_new_name()
            .map(|name| Selection::Create(self.labs_path.join(name)))
    }

    /// Move the selection down one row, clamped at the end.
    pub fn move_down(&mut self) {
        let total = self.total_items();
        if total > 0 {
            self.cursor_pos = (self.cursor_pos + 1).min(total - 1);
        }
        self.ensure_cursor_visible();
    }

    /// Move the selection up one row, clamped at the start.
    pub fn move_up(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
        self.ensure_cursor_visible();
    }

    /// Jump to the first selectable row.
    pub fn move_to_top(&mut self) {
        self.cursor_pos = 0;
        self.scroll_offset = 0;
    }

    /// Jump to the last selectable row.
    pub fn move_to_bottom(&mut self) {
        self.cursor_pos = self.total_items().saturating_sub(1);
        self.ensure_cursor_visible();
    }

    /// Move the selection up by one visible page.
    pub fn page_up(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(self.visible_result_limit());
        self.ensure_cursor_visible();
    }

    /// Move the selection down by one visible page.
    pub fn page_down(&mut self) {
        let total = self.total_items();
        if total == 0 {
            self.cursor_pos = 0;
        } else {
            self.cursor_pos = (self.cursor_pos + self.visible_result_limit()).min(total - 1);
        }
        self.ensure_cursor_visible();
    }

    /// Insert a printable character into the input buffer.
    pub fn insert_char(&mut self, character: char) {
        if !is_allowed_input_char(character) {
            return;
        }

        let byte_pos = self.char_to_byte_pos(self.input_cursor_pos);
        self.input.insert(byte_pos, character);
        self.input_cursor_pos += 1;
        self.on_query_changed();
    }

    /// Delete the character before the input cursor.
    pub fn backspace(&mut self) {
        if self.input_cursor_pos == 0 {
            return;
        }

        self.input_cursor_pos -= 1;
        let byte_pos = self.char_to_byte_pos(self.input_cursor_pos);
        self.input.remove(byte_pos);
        self.on_query_changed();
    }

    /// Move the input cursor to the start.
    pub fn move_input_to_start(&mut self) {
        self.input_cursor_pos = 0;
    }

    /// Move the input cursor to the end.
    pub fn move_input_to_end(&mut self) {
        self.input_cursor_pos = self.input.chars().count();
    }

    /// Move the input cursor back one character.
    pub fn move_input_back(&mut self) {
        self.input_cursor_pos = self.input_cursor_pos.saturating_sub(1);
    }

    /// Move the input cursor forward one character.
    pub fn move_input_forward(&mut self) {
        self.input_cursor_pos = (self.input_cursor_pos + 1).min(self.input.chars().count());
    }

    /// Delete from the input cursor to the end of the line.
    pub fn kill_to_end(&mut self) {
        let byte_pos = self.char_to_byte_pos(self.input_cursor_pos);
        self.input.truncate(byte_pos);
        self.on_query_changed();
    }

    /// Delete the previous word from the input buffer.
    pub fn delete_word_backward(&mut self) {
        if self.input_cursor_pos == 0 {
            return;
        }

        let chars: Vec<char> = self.input.chars().collect();
        let mut new_cursor = self.input_cursor_pos;

        while new_cursor > 0 && !chars[new_cursor - 1].is_ascii_alphanumeric() {
            new_cursor -= 1;
        }

        while new_cursor > 0 && chars[new_cursor - 1].is_ascii_alphanumeric() {
            new_cursor -= 1;
        }

        let start_byte = self.char_to_byte_pos(new_cursor);
        let end_byte = self.char_to_byte_pos(self.input_cursor_pos);
        self.input.replace_range(start_byte..end_byte, "");
        self.input_cursor_pos = new_cursor;
        self.on_query_changed();
    }

    /// Recompute the fuzzy-filtered results based on the current input.
    pub fn refresh_filtered(&mut self) {
        let fuzzy = Fuzzy::new(&self.entries);
        self.filtered = fuzzy.match_entries(&self.input, self.entries.len());
        self.clamp_cursor();
        self.ensure_cursor_visible();
    }

    fn on_query_changed(&mut self) {
        self.cursor_pos = 0;
        self.scroll_offset = 0;
        self.refresh_filtered();
    }

    fn clamp_cursor(&mut self) {
        let total = self.total_items();
        if total == 0 {
            self.cursor_pos = 0;
        } else {
            self.cursor_pos = self.cursor_pos.min(total - 1);
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let visible = self.visible_result_limit();
        if self.cursor_pos < self.scroll_offset {
            self.scroll_offset = self.cursor_pos;
        } else if self.cursor_pos >= self.scroll_offset + visible {
            self.scroll_offset = self.cursor_pos - visible + 1;
        }
    }

    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_pos)
            .map(|(index, _)| index)
            .unwrap_or(self.input.len())
    }
}

fn is_allowed_input_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ' ')
}

fn resolve_dimension(override_value: Option<&str>, detected: u16, default: u16) -> u16 {
    override_value
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| detected.max(default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, time::SystemTime};

    fn make_entry(name: &str, score: f64) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: score,
        }
    }

    fn make_app(input: Option<&str>) -> App {
        App::new(
            "/tmp/labs",
            vec![
                make_entry("2025-01-01-alpha", 2.0),
                make_entry("beta", 1.0),
                make_entry("gamma", 0.5),
            ],
            input,
            TerminalSize::new(80, 24),
        )
    }

    #[test]
    fn test_app_prefills_input_from_and_type() {
        let app = make_app(Some("beta"));
        assert_eq!(app.input, "beta");
        assert_eq!(app.input_cursor_pos, 4);
    }

    #[test]
    fn test_create_new_name_normalizes_spaces_only_when_emitting() {
        let app = make_app(Some("new project"));
        let new_name = app.create_new_name().expect("create-new name");
        assert!(new_name.ends_with("new-project"));
        assert_eq!(app.input, "new project");
    }

    #[test]
    fn test_insert_and_backspace_update_query_and_reset_selection() {
        let mut app = make_app(None);
        app.cursor_pos = 2;
        app.insert_char('b');
        app.insert_char('e');
        app.backspace();

        assert_eq!(app.input, "b");
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_move_down_clamps_to_total_items() {
        let mut app = make_app(None);
        for _ in 0..10 {
            app.move_down();
        }

        assert_eq!(app.cursor_pos, app.total_items() - 1);
    }

    #[test]
    fn test_page_navigation_uses_visible_result_limit() {
        let mut app = App::new(
            "/tmp/labs",
            (0..20)
                .map(|index| make_entry(&format!("entry-{index:02}"), 20.0 - index as f64))
                .collect(),
            None,
            TerminalSize::new(80, 10),
        );

        app.page_down();
        assert_eq!(app.cursor_pos, app.visible_result_limit());
        app.page_up();
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_visible_result_limit_matches_header_body_footer_layout() {
        let app = App::new(
            "/tmp/labs",
            vec![make_entry("alpha", 1.0)],
            None,
            TerminalSize::new(80, 10),
        );

        assert_eq!(app.visible_result_limit(), 5);

        let tiny = App::new(
            "/tmp/labs",
            vec![make_entry("alpha", 1.0)],
            None,
            TerminalSize::new(80, 6),
        );

        assert_eq!(tiny.visible_result_limit(), 3);
    }

    #[test]
    fn test_current_selection_returns_create_path_for_virtual_row() {
        let mut app = App::new(
            "/tmp/labs",
            Vec::new(),
            Some("feature work"),
            TerminalSize::new(80, 24),
        );
        app.cursor_pos = 0;

        let selection = app.current_selection().expect("selection");
        assert_eq!(
            selection,
            Selection::Create(PathBuf::from(format!(
                "/tmp/labs/{}-feature-work",
                Local::now().format("%Y-%m-%d")
            )))
        );
    }

    #[test]
    fn test_resolve_dimension_prefers_positive_override() {
        assert_eq!(resolve_dimension(Some("120"), 80, 24), 120);
        assert_eq!(resolve_dimension(Some("0"), 80, 24), 80);
        assert_eq!(resolve_dimension(Some("not-a-number"), 80, 24), 80);
    }
}
