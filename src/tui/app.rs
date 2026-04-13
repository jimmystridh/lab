//! Application state for the `lab` TUI.
//!
//! This module owns the selector state shared between rendering and input
//! handling: loaded entries, current filter results, input buffer, selection,
//! scroll position, mode, delete marks, and terminal size.

use super::dialogs::{DeleteConfirmation, GraduateDialog, RenameDialog};
use crate::{
    entries::{has_date_prefix, Entry},
    fuzzy::{Fuzzy, MatchResult},
};
use chrono::Local;
use crossterm::terminal;
use std::{
    collections::HashSet,
    env, fs,
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
        Self::from_detected(detected_width, detected_height)
    }

    /// Build a terminal size from detected dimensions while honoring
    /// LAB_WIDTH/LAB_HEIGHT overrides.
    pub fn from_detected(width: u16, height: u16) -> Self {
        Self::new(
            resolve_dimension(env::var("LAB_WIDTH").ok().as_deref(), width, 80),
            resolve_dimension(env::var("LAB_HEIGHT").ok().as_deref(), height, 24),
        )
    }
}

/// Selector mode.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal entry-selection mode.
    Normal,
    /// Delete confirmation dialog.
    DeleteConfirm,
    /// Rename dialog.
    Rename,
    /// Graduate dialog.
    Graduate,
}

/// Selection outcome derived from the current cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Existing entry selected.
    Existing(PathBuf),
    /// Virtual "create new" row selected.
    Create(PathBuf),
}

/// Confirmed batch delete selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteSelection {
    /// Canonical base path containing the marked entries.
    pub base_path: PathBuf,
    /// Basenames to delete relative to `base_path`.
    pub basenames: Vec<String>,
}

/// Confirmed rename selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameSelection {
    /// Base labs path containing the entry being renamed.
    pub base_path: PathBuf,
    /// Original basename before the rename.
    pub old_name: String,
    /// New basename after normalization and validation.
    pub new_name: String,
}

/// Confirmed graduate selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraduateSelection {
    /// Source directory to move.
    pub source: PathBuf,
    /// Destination project directory after graduation.
    pub dest: PathBuf,
    /// Original basename inside the labs directory.
    pub basename: String,
    /// Base labs path containing the source entry.
    pub base_path: PathBuf,
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
    /// Delete confirmation dialog state when active.
    pub delete_confirmation: Option<DeleteConfirmation>,
    /// Rename dialog state when active.
    pub rename_dialog: Option<RenameDialog>,
    /// Graduate dialog state when active.
    pub graduate_dialog: Option<GraduateDialog>,
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
            delete_confirmation: None,
            rename_dialog: None,
            graduate_dialog: None,
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
        Some(format!(
            "{}-{}",
            date_prefix,
            normalize_create_name_fragment(&self.input)
        ))
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

    /// Return the selected real entry index, if the cursor is on an entry row.
    pub fn current_entry_index(&self) -> Option<usize> {
        if self.cursor_pos < self.filtered.len() {
            Some(self.filtered[self.cursor_pos].index)
        } else {
            None
        }
    }

    /// Whether delete mode is active because at least one entry is marked.
    pub fn is_delete_mode(&self) -> bool {
        !self.marks.is_empty() && self.mode == Mode::Normal
    }

    /// Whether the delete confirmation dialog is active.
    pub fn is_confirming_delete(&self) -> bool {
        self.mode == Mode::DeleteConfirm
    }

    /// Whether the rename dialog is active.
    pub fn is_renaming(&self) -> bool {
        self.mode == Mode::Rename
    }

    /// Whether the graduate dialog is active.
    pub fn is_graduating(&self) -> bool {
        self.mode == Mode::Graduate
    }

    /// Return the number of marked entries.
    pub fn marked_count(&self) -> usize {
        self.marks.len()
    }

    /// Return the marked entries in their current visible order.
    pub fn marked_entries(&self) -> Vec<&Entry> {
        self.ordered_marked_entry_indices()
            .into_iter()
            .filter_map(|index| self.entries.get(index))
            .collect()
    }

    /// Toggle the delete mark on the currently selected entry.
    pub fn toggle_delete_mark(&mut self) {
        let Some(index) = self.current_entry_index() else {
            return;
        };

        if !self.marks.insert(index) {
            self.marks.remove(&index);
        }

        self.mode = Mode::Normal;
        self.delete_confirmation = None;
    }

    /// Clear all delete marks and close any active delete confirmation dialog.
    pub fn clear_delete_marks(&mut self) {
        self.marks.clear();
        self.mode = Mode::Normal;
        self.delete_confirmation = None;
    }

    /// Enter the delete confirmation dialog when marks are present.
    pub fn begin_delete_confirmation(&mut self) {
        if self.marks.is_empty() {
            return;
        }

        self.mode = Mode::DeleteConfirm;
        self.delete_confirmation = Some(DeleteConfirmation::new());
    }

    /// Submit the delete confirmation dialog.
    ///
    /// Returns a validated delete selection when the input exactly matches `YES`.
    /// Any other input cancels delete mode and clears marks.
    pub fn submit_delete_confirmation(&mut self) -> Option<DeleteSelection> {
        if !self.is_confirming_delete() {
            return None;
        }

        let confirmed = self
            .delete_confirmation
            .as_ref()
            .is_some_and(DeleteConfirmation::is_confirmed);
        let marked_indices = self.ordered_marked_entry_indices();
        let selection = if confirmed {
            self.build_delete_selection(&marked_indices)
        } else {
            None
        };

        self.clear_delete_marks();
        selection
    }

    /// Enter the rename dialog for the currently selected real entry.
    pub fn begin_rename(&mut self) {
        let Some(index) = self.current_entry_index() else {
            return;
        };

        self.clear_delete_marks();
        self.mode = Mode::Rename;
        self.rename_dialog = Some(RenameDialog::new(self.entries[index].name.clone()));
    }

    /// Cancel the active rename dialog, if any.
    pub fn cancel_rename(&mut self) {
        self.mode = Mode::Normal;
        self.rename_dialog = None;
    }

    /// Validate and submit the active rename dialog.
    pub fn submit_rename(&mut self) -> Result<Option<RenameSelection>, String> {
        if !self.is_renaming() {
            return Ok(None);
        }

        let Some(dialog) = self.rename_dialog.as_ref() else {
            self.mode = Mode::Normal;
            return Ok(None);
        };

        let old_name = dialog.current_name.clone();
        let new_name = dialog.normalized_name();

        if new_name.is_empty() {
            return self.rename_error("Name cannot be empty");
        }
        if new_name.contains('/') {
            return self.rename_error("Name cannot contain /");
        }
        if new_name == old_name {
            self.cancel_rename();
            return Ok(None);
        }
        if self.labs_path.join(&new_name).is_dir() {
            return self.rename_error(format!("Directory exists: {new_name}"));
        }

        let selection = RenameSelection {
            base_path: self.labs_path.clone(),
            old_name,
            new_name,
        };
        self.cancel_rename();
        Ok(Some(selection))
    }

    /// Enter the graduate dialog for the currently selected real entry.
    pub fn begin_graduate(&mut self) {
        let Some(index) = self.current_entry_index() else {
            return;
        };

        self.clear_delete_marks();
        let current_name = self.entries[index].name.clone();
        let lab_projects = env::var("LAB_PROJECTS").ok();
        let (destination, destination_hint, destination_root) =
            default_graduate_destination(&self.labs_path, &current_name, lab_projects.as_deref());
        self.mode = Mode::Graduate;
        self.graduate_dialog = Some(GraduateDialog::new(
            current_name,
            destination.to_string_lossy(),
            destination_hint,
            destination_root.to_string_lossy(),
        ));
    }

    /// Cancel the active graduate dialog, if any.
    pub fn cancel_graduate(&mut self) {
        self.mode = Mode::Normal;
        self.graduate_dialog = None;
    }

    /// Validate and submit the active graduate dialog.
    pub fn submit_graduate(&mut self) -> Result<Option<GraduateSelection>, String> {
        if !self.is_graduating() {
            return Ok(None);
        }

        let Some(dialog) = self.graduate_dialog.as_ref() else {
            self.mode = Mode::Normal;
            return Ok(None);
        };
        let Some(index) = self.current_entry_index() else {
            self.cancel_graduate();
            return Ok(None);
        };

        let input = dialog.input.clone();
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return self.graduate_error("Destination cannot be empty");
        }

        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let dest = expand_input_path(trimmed, &cwd);
        if dest.exists() {
            return self.graduate_error(format!(
                "Destination already exists: {}",
                dest.display()
            ));
        }

        let parent = dest.parent().unwrap_or_else(|| Path::new("/"));
        if !parent.is_dir() {
            return self.graduate_error(format!(
                "Parent directory does not exist: {}",
                parent.display()
            ));
        }

        let selection = GraduateSelection {
            source: self.entries[index].path.clone(),
            dest,
            basename: self.entries[index].name.clone(),
            base_path: self.labs_path.clone(),
        };
        self.cancel_graduate();
        Ok(Some(selection))
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
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.insert_char(character);
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.insert_char(character);
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.insert_char(character);
            }
            return;
        }

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
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.backspace();
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.backspace();
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.backspace();
            }
            return;
        }

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
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.move_to_start();
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.move_to_start();
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.move_to_start();
            }
            return;
        }

        self.input_cursor_pos = 0;
    }

    /// Move the input cursor to the end.
    pub fn move_input_to_end(&mut self) {
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.move_to_end();
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.move_to_end();
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.move_to_end();
            }
            return;
        }

        self.input_cursor_pos = self.input.chars().count();
    }

    /// Move the input cursor back one character.
    pub fn move_input_back(&mut self) {
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.move_back();
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.move_back();
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.move_back();
            }
            return;
        }

        self.input_cursor_pos = self.input_cursor_pos.saturating_sub(1);
    }

    /// Move the input cursor forward one character.
    pub fn move_input_forward(&mut self) {
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.move_forward();
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.move_forward();
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.move_forward();
            }
            return;
        }

        self.input_cursor_pos = (self.input_cursor_pos + 1).min(self.input.chars().count());
    }

    /// Delete from the input cursor to the end of the line.
    pub fn kill_to_end(&mut self) {
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.kill_to_end();
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.kill_to_end();
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.kill_to_end();
            }
            return;
        }

        let byte_pos = self.char_to_byte_pos(self.input_cursor_pos);
        if byte_pos >= self.input.len() {
            return;
        }

        self.input.truncate(byte_pos);
        self.on_query_changed();
    }

    /// Delete the previous word from the input buffer.
    pub fn delete_word_backward(&mut self) {
        if self.is_renaming() {
            if let Some(dialog) = self.rename_dialog.as_mut() {
                dialog.delete_word_backward();
            }
            return;
        }

        if self.is_graduating() {
            if let Some(dialog) = self.graduate_dialog.as_mut() {
                dialog.delete_word_backward();
            }
            return;
        }

        if self.is_confirming_delete() {
            if let Some(dialog) = self.delete_confirmation.as_mut() {
                dialog.delete_word_backward();
            }
            return;
        }

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

    fn build_delete_selection(&self, marked_indices: &[usize]) -> Option<DeleteSelection> {
        if marked_indices.is_empty() {
            return None;
        }

        let base_path = fs::canonicalize(&self.labs_path).ok()?;
        let mut basenames = Vec::with_capacity(marked_indices.len());

        for &index in marked_indices {
            let entry = self.entries.get(index)?;
            let target_real = fs::canonicalize(&entry.path).ok()?;
            if target_real == base_path || !target_real.starts_with(&base_path) {
                return None;
            }

            basenames.push(entry.name.clone());
        }

        Some(DeleteSelection {
            base_path,
            basenames,
        })
    }

    fn ordered_marked_entry_indices(&self) -> Vec<usize> {
        let mut ordered = Vec::with_capacity(self.marks.len());
        let mut seen = HashSet::with_capacity(self.marks.len());

        for result in &self.filtered {
            if self.marks.contains(&result.index) {
                ordered.push(result.index);
                seen.insert(result.index);
            }
        }

        let mut remaining = self
            .marks
            .iter()
            .copied()
            .filter(|index| !seen.contains(index))
            .collect::<Vec<_>>();
        remaining.sort_unstable_by(|left, right| {
            self.entries[*left].name.cmp(&self.entries[*right].name)
        });
        ordered.extend(remaining);

        ordered
    }

    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_pos)
            .map(|(index, _)| index)
            .unwrap_or(self.input.len())
    }

    fn rename_error(
        &mut self,
        message: impl Into<String>,
    ) -> Result<Option<RenameSelection>, String> {
        let message = message.into();
        if let Some(dialog) = self.rename_dialog.as_mut() {
            dialog.set_error(message.clone());
        }
        Err(message)
    }

    fn graduate_error(
        &mut self,
        message: impl Into<String>,
    ) -> Result<Option<GraduateSelection>, String> {
        let message = message.into();
        if let Some(dialog) = self.graduate_dialog.as_mut() {
            dialog.set_error(message.clone());
        }
        Err(message)
    }
}

fn is_allowed_input_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ' ')
}

fn normalize_create_name_fragment(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut previous_was_whitespace = false;

    for character in input.chars() {
        if character.is_whitespace() {
            if !previous_was_whitespace {
                normalized.push('-');
                previous_was_whitespace = true;
            }
        } else {
            normalized.push(character);
            previous_was_whitespace = false;
        }
    }

    normalized
}

fn graduate_project_name(entry_name: &str) -> String {
    if has_date_prefix(entry_name) && entry_name.len() > 11 {
        entry_name.get(11..).unwrap_or_default().to_string()
    } else {
        entry_name.to_string()
    }
}

fn default_graduate_destination(
    labs_path: &Path,
    entry_name: &str,
    lab_projects: Option<&str>,
) -> (PathBuf, String, PathBuf) {
    let project_name = graduate_project_name(entry_name);
    let (projects_dir, destination_hint) = resolve_graduate_projects_dir(labs_path, lab_projects);
    (
        projects_dir.join(project_name),
        destination_hint,
        projects_dir,
    )
}

fn resolve_graduate_projects_dir(
    labs_path: &Path,
    lab_projects: Option<&str>,
) -> (PathBuf, String) {
    if let Some(path) = lab_projects.filter(|value| !value.is_empty()) {
        return (
            PathBuf::from(expand_tilde_path(path)),
            "$LAB_PROJECTS".to_string(),
        );
    }

    (
        labs_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| labs_path.to_path_buf()),
        "parent of $LAB_PATH".to_string(),
    )
}

fn expand_tilde_path(path: &str) -> String {
    if path == "~" || path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }

    path.to_string()
}

fn expand_input_path(input: &str, cwd: &Path) -> PathBuf {
    let expanded = PathBuf::from(expand_tilde_path(input));
    if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    }
}

fn resolve_dimension(override_value: Option<&str>, detected: u16, default: u16) -> u16 {
    override_value
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(if detected > 0 { detected } else { default })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::{Path, PathBuf}, time::SystemTime};

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

    fn make_scrolling_app(entry_count: usize, height: u16) -> App {
        App::new(
            "/tmp/labs",
            (0..entry_count)
                .map(|index| {
                    make_entry(
                        &format!("entry-{index:02}"),
                        entry_count as f64 - index as f64,
                    )
                })
                .collect(),
            None,
            TerminalSize::new(80, height),
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
    fn test_create_new_name_collapses_consecutive_spaces_to_single_dash() {
        let app = make_app(Some("new  project"));
        let new_name = app.create_new_name().expect("create-new name");
        assert!(new_name.ends_with("new-project"));
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
    fn test_move_up_clamps_to_zero() {
        let mut app = make_app(None);
        app.move_down();
        app.move_up();
        app.move_up();

        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_move_to_top_and_bottom_clamp_to_bounds() {
        let mut app = make_scrolling_app(8, 8);

        app.move_to_bottom();
        assert_eq!(app.cursor_pos, 7);
        assert_eq!(app.scroll_offset, 5);

        app.move_to_top();
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_page_navigation_uses_visible_result_limit() {
        let mut app = make_scrolling_app(20, 10);

        app.page_down();
        assert_eq!(app.cursor_pos, app.visible_result_limit());
        app.page_up();
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_page_navigation_clamps_at_bounds() {
        let mut app = make_scrolling_app(5, 8);

        app.page_up();
        assert_eq!(app.cursor_pos, 0);

        app.move_to_bottom();
        app.page_down();
        assert_eq!(app.cursor_pos, app.total_items() - 1);
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
    fn test_scroll_offset_tracks_cursor_visibility() {
        let mut app = make_scrolling_app(8, 8);

        app.move_down();
        app.move_down();
        assert_eq!(app.cursor_pos, 2);
        assert_eq!(app.scroll_offset, 0);

        app.move_down();
        assert_eq!(app.cursor_pos, 3);
        assert_eq!(app.scroll_offset, 1);

        app.move_down();
        assert_eq!(app.cursor_pos, 4);
        assert_eq!(app.scroll_offset, 2);

        app.move_up();
        app.move_up();
        app.move_up();
        assert_eq!(app.cursor_pos, 1);
        assert_eq!(app.scroll_offset, 1);

        app.move_up();
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
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

    #[test]
    fn test_resolve_dimension_keeps_detected_size_when_narrower_than_default() {
        assert_eq!(resolve_dimension(None, 40, 80), 40);
        assert_eq!(resolve_dimension(None, 10, 24), 10);
    }

    #[test]
    fn test_resolve_dimension_falls_back_to_default_when_detection_is_zero() {
        assert_eq!(resolve_dimension(None, 0, 80), 80);
        assert_eq!(resolve_dimension(None, 0, 24), 24);
    }

    #[test]
    fn test_fifty_downs_on_five_item_list_clamps_to_last_entry() {
        let mut app = make_scrolling_app(5, 24);
        for _ in 0..50 {
            app.move_down();
        }

        assert_eq!(app.cursor_pos, 4);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_kill_to_end_only_resets_selection_when_input_changes() {
        let mut app = make_app(Some("beta"));
        app.cursor_pos = 2;
        app.scroll_offset = 1;

        app.move_input_to_end();
        app.kill_to_end();
        assert_eq!(app.input, "beta");
        assert_eq!(app.cursor_pos, 2);
        assert_eq!(app.scroll_offset, 1);

        app.move_input_to_start();
        app.kill_to_end();
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_toggle_delete_mark_marks_and_unmarks_current_entry() {
        let mut app = make_app(None);

        app.move_down();
        app.toggle_delete_mark();
        assert_eq!(app.marks, HashSet::from([1]));
        assert!(app.is_delete_mode());

        app.toggle_delete_mark();
        assert!(app.marks.is_empty());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_begin_delete_confirmation_and_submit_yes_returns_selection() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("alpha")).expect("mkdir alpha");
        let entry = Entry {
            name: "alpha".to_string(),
            path: dir.path().join("alpha"),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));

        app.toggle_delete_mark();
        app.begin_delete_confirmation();
        assert!(app.is_confirming_delete());

        for character in ['Y', 'E', 'S'] {
            app.insert_char(character);
        }

        let selection = app.submit_delete_confirmation().expect("delete selection");
        assert_eq!(
            selection.base_path,
            fs::canonicalize(dir.path()).expect("base realpath")
        );
        assert_eq!(selection.basenames, vec!["alpha".to_string()]);
        assert!(app.marks.is_empty());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_submit_delete_confirmation_non_yes_cancels_and_clears_marks() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("alpha")).expect("mkdir alpha");
        let entry = Entry {
            name: "alpha".to_string(),
            path: dir.path().join("alpha"),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));

        app.toggle_delete_mark();
        app.begin_delete_confirmation();
        for character in ['y', 'e', 's'] {
            app.insert_char(character);
        }

        assert!(app.submit_delete_confirmation().is_none());
        assert!(app.marks.is_empty());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_submit_delete_confirmation_rejects_targets_outside_base_path() {
        let base = tempfile::tempdir().expect("base tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let entry = Entry {
            name: "outside-link".to_string(),
            path: outside.path().to_path_buf(),
            is_symlink: true,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(base.path(), vec![entry], None, TerminalSize::new(80, 24));

        app.toggle_delete_mark();
        app.begin_delete_confirmation();
        for character in ['Y', 'E', 'S'] {
            app.insert_char(character);
        }

        assert!(app.submit_delete_confirmation().is_none());
        assert!(app.marks.is_empty());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_begin_rename_prefills_current_name_and_clears_delete_marks() {
        let mut app = make_app(None);
        app.move_down();
        app.toggle_delete_mark();

        app.begin_rename();

        assert!(app.marks.is_empty());
        assert!(app.is_renaming());
        let dialog = app.rename_dialog.as_ref().expect("rename dialog");
        assert_eq!(dialog.current_name, "beta");
        assert_eq!(dialog.input, "beta");
        assert_eq!(dialog.cursor_pos, 4);
    }

    #[test]
    fn test_begin_rename_on_create_new_row_does_nothing() {
        let mut app = App::new(
            "/tmp/labs",
            Vec::new(),
            Some("new project"),
            TerminalSize::new(80, 24),
        );

        app.begin_rename();

        assert!(!app.is_renaming());
        assert!(app.rename_dialog.is_none());
    }

    #[test]
    fn test_begin_graduate_prefills_stripped_destination_in_parent_of_labs_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry = Entry {
            name: "2025-06-01-my-experiment".to_string(),
            path: dir.path().join("2025-06-01-my-experiment"),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));

        app.begin_graduate();

        assert!(app.is_graduating());
        let dialog = app.graduate_dialog.as_ref().expect("graduate dialog");
        assert_eq!(dialog.current_name, "2025-06-01-my-experiment");
        assert_eq!(
            dialog.input,
            dir.path()
                .parent()
                .expect("parent")
                .join("my-experiment")
                .to_string_lossy()
        );
        assert_eq!(dialog.destination_hint, "parent of $LAB_PATH");
        assert_eq!(
            dialog.destination_root,
            dir.path().parent().expect("parent").to_string_lossy()
        );
    }

    #[test]
    fn test_begin_graduate_without_date_prefix_keeps_full_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry = Entry {
            name: "plain-project".to_string(),
            path: dir.path().join("plain-project"),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));

        app.begin_graduate();

        let dialog = app.graduate_dialog.as_ref().expect("graduate dialog");
        assert!(dialog.input.ends_with("/plain-project"));
    }

    #[test]
    fn test_default_graduate_destination_uses_lab_projects_override() {
        let labs_path = Path::new("/tmp/labs");
        let (destination, hint, root) = default_graduate_destination(
            labs_path,
            "2025-06-01-my-experiment",
            Some("/tmp/projects"),
        );

        assert_eq!(destination, PathBuf::from("/tmp/projects/my-experiment"));
        assert_eq!(hint, "$LAB_PROJECTS");
        assert_eq!(root, PathBuf::from("/tmp/projects"));
    }

    #[test]
    fn test_submit_rename_same_name_exits_dialog_without_selection() {
        let mut app = make_app(None);
        app.begin_rename();

        let result = app.submit_rename().expect("rename result");

        assert!(result.is_none());
        assert!(!app.is_renaming());
        assert!(app.rename_dialog.is_none());
    }

    #[test]
    fn test_submit_rename_rejects_empty_name_and_stays_in_dialog() {
        let mut app = make_app(None);
        app.begin_rename();
        app.move_input_to_start();
        app.kill_to_end();

        let error = app.submit_rename().expect_err("rename error");

        assert_eq!(error, "Name cannot be empty");
        assert!(app.is_renaming());
        assert_eq!(
            app.rename_dialog
                .as_ref()
                .and_then(|dialog| dialog.error.clone()),
            Some("Name cannot be empty".to_string())
        );
    }

    #[test]
    fn test_submit_rename_normalizes_spaces_to_dashes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry = Entry {
            name: "alpha".to_string(),
            path: dir.path().join("alpha"),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));
        app.begin_rename();
        app.move_input_to_start();
        app.kill_to_end();
        for character in "new  name".chars() {
            app.insert_char(character);
        }

        let selection = app
            .submit_rename()
            .expect("rename result")
            .expect("rename selection");

        assert_eq!(selection.old_name, "alpha");
        assert_eq!(selection.new_name, "new-name");
        assert_eq!(selection.base_path, dir.path());
        assert!(!app.is_renaming());
    }

    #[test]
    fn test_submit_rename_rejects_existing_directory_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("alpha")).expect("mkdir alpha");
        fs::create_dir(dir.path().join("beta")).expect("mkdir beta");
        let mut app = App::new(
            dir.path(),
            vec![
                Entry {
                    name: "alpha".to_string(),
                    path: dir.path().join("alpha"),
                    is_symlink: false,
                    mtime: SystemTime::now(),
                    base_score: 2.0,
                },
                Entry {
                    name: "beta".to_string(),
                    path: dir.path().join("beta"),
                    is_symlink: false,
                    mtime: SystemTime::now(),
                    base_score: 1.0,
                },
            ],
            None,
            TerminalSize::new(80, 24),
        );
        app.begin_rename();
        app.move_input_to_start();
        app.kill_to_end();
        for character in "beta".chars() {
            app.insert_char(character);
        }

        let error = app.submit_rename().expect_err("rename error");

        assert_eq!(error, "Directory exists: beta");
        assert!(app.is_renaming());
        assert_eq!(
            app.rename_dialog
                .as_ref()
                .and_then(|dialog| dialog.error.clone()),
            Some("Directory exists: beta".to_string())
        );
    }

    #[test]
    fn test_begin_graduate_on_create_new_row_does_nothing() {
        let mut app = App::new(
            "/tmp/labs",
            Vec::new(),
            Some("new project"),
            TerminalSize::new(80, 24),
        );

        app.begin_graduate();

        assert!(!app.is_graduating());
        assert!(app.graduate_dialog.is_none());
    }

    #[test]
    fn test_submit_graduate_rejects_empty_destination_and_stays_in_dialog() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry = Entry {
            name: "2025-06-01-alpha".to_string(),
            path: dir.path().join("2025-06-01-alpha"),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));
        app.begin_graduate();
        if let Some(dialog) = app.graduate_dialog.as_mut() {
            dialog.input = "   ".to_string();
            dialog.cursor_pos = 3;
        }

        let error = app.submit_graduate().expect_err("graduate error");

        assert_eq!(error, "Destination cannot be empty");
        assert!(app.is_graduating());
        assert_eq!(
            app.graduate_dialog
                .as_ref()
                .and_then(|dialog| dialog.error.clone()),
            Some("Destination cannot be empty".to_string())
        );
    }

    #[test]
    fn test_submit_graduate_rejects_existing_destination() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry_path = dir.path().join("2025-06-01-alpha");
        fs::create_dir(&entry_path).expect("mkdir source");
        let dest_dir = dir.path().join("projects");
        fs::create_dir(&dest_dir).expect("mkdir projects");
        let dest_path = dest_dir.join("alpha");
        fs::create_dir(&dest_path).expect("mkdir destination");

        let entry = Entry {
            name: "2025-06-01-alpha".to_string(),
            path: entry_path,
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));
        app.begin_graduate();
        if let Some(dialog) = app.graduate_dialog.as_mut() {
            dialog.input = dest_path.to_string_lossy().into_owned();
            dialog.cursor_pos = dialog.input.chars().count();
        }

        let error = app.submit_graduate().expect_err("graduate error");

        assert_eq!(
            error,
            format!("Destination already exists: {}", dest_path.display())
        );
        assert!(app.is_graduating());
    }

    #[test]
    fn test_submit_graduate_rejects_missing_parent_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry = Entry {
            name: "2025-06-01-alpha".to_string(),
            path: dir.path().join("2025-06-01-alpha"),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));
        app.begin_graduate();
        let missing_parent = dir.path().join("missing-parent").join("alpha");
        if let Some(dialog) = app.graduate_dialog.as_mut() {
            dialog.input = missing_parent.to_string_lossy().into_owned();
            dialog.cursor_pos = dialog.input.chars().count();
        }

        let error = app.submit_graduate().expect_err("graduate error");

        assert_eq!(
            error,
            format!(
                "Parent directory does not exist: {}",
                missing_parent.parent().expect("parent").display()
            )
        );
        assert!(app.is_graduating());
    }

    #[test]
    fn test_submit_graduate_returns_selection_and_closes_dialog() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry_path = dir.path().join("2025-06-01-my-experiment");
        fs::create_dir(&entry_path).expect("mkdir source");
        let projects_dir = dir.path().join("projects");
        fs::create_dir(&projects_dir).expect("mkdir projects");
        let destination = projects_dir.join("graduated-project");

        let entry = Entry {
            name: "2025-06-01-my-experiment".to_string(),
            path: entry_path.clone(),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));
        app.begin_graduate();
        if let Some(dialog) = app.graduate_dialog.as_mut() {
            dialog.input = destination.to_string_lossy().into_owned();
            dialog.cursor_pos = dialog.input.chars().count();
        }

        let selection = app
            .submit_graduate()
            .expect("graduate result")
            .expect("graduate selection");

        assert_eq!(selection.source, entry_path);
        assert_eq!(selection.dest, destination);
        assert_eq!(selection.basename, "2025-06-01-my-experiment");
        assert_eq!(selection.base_path, dir.path());
        assert!(!app.is_graduating());
        assert!(app.graduate_dialog.is_none());
    }
}
