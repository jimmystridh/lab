//! Dialog state and line-editing support for TUI modal dialogs.

/// Editable state for the delete confirmation dialog.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeleteConfirmation {
    /// Current confirmation input.
    pub input: String,
    /// Cursor position within the confirmation input, in chars.
    pub cursor_pos: usize,
}

impl DeleteConfirmation {
    /// Create a blank delete confirmation input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the current input exactly confirms deletion.
    pub fn is_confirmed(&self) -> bool {
        self.input == "YES"
    }

    /// Insert a printable character at the cursor.
    pub fn insert_char(&mut self, character: char) {
        if !is_printable(character) {
            return;
        }

        let byte_pos = self.char_to_byte_pos(self.cursor_pos);
        self.input.insert(byte_pos, character);
        self.cursor_pos += 1;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        self.cursor_pos -= 1;
        let byte_pos = self.char_to_byte_pos(self.cursor_pos);
        self.input.remove(byte_pos);
    }

    /// Move the cursor to the start of the input.
    pub fn move_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move the cursor to the end of the input.
    pub fn move_to_end(&mut self) {
        self.cursor_pos = self.input.chars().count();
    }

    /// Move the cursor backward by one character.
    pub fn move_back(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    /// Move the cursor forward by one character.
    pub fn move_forward(&mut self) {
        self.cursor_pos = (self.cursor_pos + 1).min(self.input.chars().count());
    }

    /// Delete everything from the cursor to the end of the input.
    pub fn kill_to_end(&mut self) {
        let byte_pos = self.char_to_byte_pos(self.cursor_pos);
        if byte_pos >= self.input.len() {
            return;
        }

        self.input.truncate(byte_pos);
    }

    /// Delete the previous word from the input.
    pub fn delete_word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        delete_word_backward(&mut self.input, &mut self.cursor_pos);
    }

    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        char_to_byte_pos(&self.input, char_pos)
    }
}

/// Editable state for the rename dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameDialog {
    /// The currently selected entry basename before editing.
    pub current_name: String,
    /// Current editable rename input.
    pub input: String,
    /// Cursor position within the rename input, in chars.
    pub cursor_pos: usize,
    /// Current validation error, if any.
    pub error: Option<String>,
}

impl RenameDialog {
    /// Create a rename dialog pre-filled with the current entry basename.
    pub fn new(current_name: impl Into<String>) -> Self {
        let current_name = current_name.into();
        let cursor_pos = current_name.chars().count();
        Self {
            input: current_name.clone(),
            current_name,
            cursor_pos,
            error: None,
        }
    }

    /// Insert an allowed rename character at the cursor.
    pub fn insert_char(&mut self, character: char) {
        if !is_allowed_rename_char(character) {
            return;
        }

        let byte_pos = char_to_byte_pos(&self.input, self.cursor_pos);
        self.input.insert(byte_pos, character);
        self.cursor_pos += 1;
        self.clear_error();
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_pos = char_to_byte_pos(&self.input, self.cursor_pos);
            self.input.remove(byte_pos);
        }
        self.clear_error();
    }

    /// Move the cursor to the start of the input.
    pub fn move_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move the cursor to the end of the input.
    pub fn move_to_end(&mut self) {
        self.cursor_pos = self.input.chars().count();
    }

    /// Move the cursor backward by one character.
    pub fn move_back(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    /// Move the cursor forward by one character.
    pub fn move_forward(&mut self) {
        self.cursor_pos = (self.cursor_pos + 1).min(self.input.chars().count());
    }

    /// Delete everything from the cursor to the end of the input.
    pub fn kill_to_end(&mut self) {
        let byte_pos = char_to_byte_pos(&self.input, self.cursor_pos);
        if byte_pos < self.input.len() {
            self.input.truncate(byte_pos);
        }
        self.clear_error();
    }

    /// Delete the previous word from the input.
    pub fn delete_word_backward(&mut self) {
        if self.cursor_pos > 0 {
            delete_word_backward(&mut self.input, &mut self.cursor_pos);
        }
        self.clear_error();
    }

    /// Set the current validation error.
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Clear the current validation error.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Return the normalized rename target used for validation and scripting.
    pub fn normalized_name(&self) -> String {
        self.input.split_whitespace().collect::<Vec<_>>().join("-")
    }
}

/// Editable state for the graduate dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraduateDialog {
    /// The currently selected entry basename before graduation.
    pub current_name: String,
    /// Current editable destination input.
    pub input: String,
    /// Cursor position within the destination input, in chars.
    pub cursor_pos: usize,
    /// Current validation error, if any.
    pub error: Option<String>,
    /// Hint describing how the default destination root was chosen.
    pub destination_hint: String,
    /// Display form of the default destination root.
    pub destination_root: String,
}

impl GraduateDialog {
    /// Create a graduate dialog pre-filled with the default destination path.
    pub fn new(
        current_name: impl Into<String>,
        input: impl Into<String>,
        destination_hint: impl Into<String>,
        destination_root: impl Into<String>,
    ) -> Self {
        let current_name = current_name.into();
        let input = input.into();
        let cursor_pos = input.chars().count();
        Self {
            current_name,
            input,
            cursor_pos,
            error: None,
            destination_hint: destination_hint.into(),
            destination_root: destination_root.into(),
        }
    }

    /// Insert an allowed destination character at the cursor.
    pub fn insert_char(&mut self, character: char) {
        if !is_allowed_graduate_char(character) {
            return;
        }

        let byte_pos = char_to_byte_pos(&self.input, self.cursor_pos);
        self.input.insert(byte_pos, character);
        self.cursor_pos += 1;
        self.clear_error();
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_pos = char_to_byte_pos(&self.input, self.cursor_pos);
            self.input.remove(byte_pos);
        }
        self.clear_error();
    }

    /// Move the cursor to the start of the input.
    pub fn move_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move the cursor to the end of the input.
    pub fn move_to_end(&mut self) {
        self.cursor_pos = self.input.chars().count();
    }

    /// Move the cursor backward by one character.
    pub fn move_back(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    /// Move the cursor forward by one character.
    pub fn move_forward(&mut self) {
        self.cursor_pos = (self.cursor_pos + 1).min(self.input.chars().count());
    }

    /// Delete everything from the cursor to the end of the input.
    pub fn kill_to_end(&mut self) {
        let byte_pos = char_to_byte_pos(&self.input, self.cursor_pos);
        if byte_pos < self.input.len() {
            self.input.truncate(byte_pos);
        }
        self.clear_error();
    }

    /// Delete the previous word from the input.
    pub fn delete_word_backward(&mut self) {
        if self.cursor_pos > 0 {
            delete_word_backward(&mut self.input, &mut self.cursor_pos);
        }
        self.clear_error();
    }

    /// Set the current validation error.
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Clear the current validation error.
    pub fn clear_error(&mut self) {
        self.error = None;
    }
}

fn is_printable(character: char) -> bool {
    !character.is_control()
}

fn is_allowed_rename_char(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '-' | '_' | '.' | '/')
        || character.is_whitespace()
}

fn is_allowed_graduate_char(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '-' | '_' | '.' | '/' | '~')
        || character.is_whitespace()
}

fn char_to_byte_pos(input: &str, char_pos: usize) -> usize {
    input
        .char_indices()
        .nth(char_pos)
        .map(|(index, _)| index)
        .unwrap_or(input.len())
}

fn delete_word_backward(input: &mut String, cursor_pos: &mut usize) {
    let chars: Vec<char> = input.chars().collect();
    let mut new_cursor = *cursor_pos;

    while new_cursor > 0 && !chars[new_cursor - 1].is_ascii_alphanumeric() {
        new_cursor -= 1;
    }

    while new_cursor > 0 && chars[new_cursor - 1].is_ascii_alphanumeric() {
        new_cursor -= 1;
    }

    let start_byte = char_to_byte_pos(input, new_cursor);
    let end_byte = char_to_byte_pos(input, *cursor_pos);
    input.replace_range(start_byte..end_byte, "");
    *cursor_pos = new_cursor;
}

#[cfg(test)]
mod tests {
    use super::{DeleteConfirmation, GraduateDialog, RenameDialog};

    #[test]
    fn test_delete_confirmation_requires_exact_yes() {
        let mut confirmation = DeleteConfirmation::new();
        for character in ['Y', 'E', 'S'] {
            confirmation.insert_char(character);
        }

        assert!(confirmation.is_confirmed());

        confirmation.insert_char('!');
        assert!(!confirmation.is_confirmed());
    }

    #[test]
    fn test_delete_confirmation_ctrl_style_cursor_movement() {
        let mut confirmation = DeleteConfirmation::new();
        for character in ['Y', 'E', 'S'] {
            confirmation.insert_char(character);
        }

        confirmation.move_to_start();
        assert_eq!(confirmation.cursor_pos, 0);

        confirmation.move_forward();
        assert_eq!(confirmation.cursor_pos, 1);

        confirmation.move_to_end();
        assert_eq!(confirmation.cursor_pos, 3);

        confirmation.move_back();
        assert_eq!(confirmation.cursor_pos, 2);
    }

    #[test]
    fn test_delete_confirmation_backspace_kill_and_word_delete() {
        let mut confirmation = DeleteConfirmation::new();
        for character in "YES maybe".chars() {
            confirmation.insert_char(character);
        }

        confirmation.delete_word_backward();
        assert_eq!(confirmation.input, "YES ");
        assert_eq!(confirmation.cursor_pos, 4);

        confirmation.backspace();
        assert_eq!(confirmation.input, "YES");
        assert_eq!(confirmation.cursor_pos, 3);

        confirmation.move_to_start();
        confirmation.kill_to_end();
        assert!(confirmation.input.is_empty());
        assert_eq!(confirmation.cursor_pos, 0);
    }

    #[test]
    fn test_rename_dialog_prefills_current_name_and_places_cursor_at_end() {
        let dialog = RenameDialog::new("2025-11-02-coolproject");

        assert_eq!(dialog.current_name, "2025-11-02-coolproject");
        assert_eq!(dialog.input, "2025-11-02-coolproject");
        assert_eq!(dialog.cursor_pos, "2025-11-02-coolproject".chars().count());
        assert_eq!(dialog.error, None);
    }

    #[test]
    fn test_rename_dialog_allows_slash_but_rejects_other_disallowed_chars() {
        let mut dialog = RenameDialog::new("alpha");
        dialog.insert_char('/');
        dialog.insert_char('!');

        assert_eq!(dialog.input, "alpha/");
        assert_eq!(dialog.cursor_pos, 6);
    }

    #[test]
    fn test_rename_dialog_ctrl_style_editing_and_error_clearing() {
        let mut dialog = RenameDialog::new("alpha project");
        dialog.set_error("Name cannot be empty");

        dialog.move_to_start();
        dialog.move_forward();
        dialog.kill_to_end();
        assert_eq!(dialog.input, "a");
        assert_eq!(dialog.cursor_pos, 1);
        assert_eq!(dialog.error, None);

        for character in " test".chars() {
            dialog.insert_char(character);
        }
        dialog.delete_word_backward();
        assert_eq!(dialog.input, "a ");
        assert_eq!(dialog.cursor_pos, 2);

        dialog.backspace();
        assert_eq!(dialog.input, "a");
        assert_eq!(dialog.cursor_pos, 1);
    }

    #[test]
    fn test_rename_dialog_normalized_name_strips_and_replaces_whitespace() {
        let mut dialog = RenameDialog::new("alpha");
        dialog.input = "  new   name\tvalue  ".to_string();

        assert_eq!(dialog.normalized_name(), "new-name-value");
    }

    #[test]
    fn test_graduate_dialog_prefills_destination_and_places_cursor_at_end() {
        let dialog = GraduateDialog::new(
            "2025-11-02-coolproject",
            "/tmp/projects/coolproject",
            "parent of $LAB_PATH",
            "/tmp/projects",
        );

        assert_eq!(dialog.current_name, "2025-11-02-coolproject");
        assert_eq!(dialog.input, "/tmp/projects/coolproject");
        assert_eq!(
            dialog.cursor_pos,
            "/tmp/projects/coolproject".chars().count()
        );
        assert_eq!(dialog.destination_hint, "parent of $LAB_PATH");
        assert_eq!(dialog.destination_root, "/tmp/projects");
        assert_eq!(dialog.error, None);
    }

    #[test]
    fn test_graduate_dialog_allows_path_chars_but_rejects_other_disallowed_chars() {
        let mut dialog = GraduateDialog::new(
            "alpha",
            "/tmp/projects/alpha",
            "$LAB_PROJECTS",
            "/tmp/projects",
        );
        dialog.insert_char('/');
        dialog.insert_char('~');
        dialog.insert_char('!');

        assert_eq!(dialog.input, "/tmp/projects/alpha/~");
        assert_eq!(dialog.cursor_pos, 21);
    }

    #[test]
    fn test_graduate_dialog_ctrl_style_editing_and_error_clearing() {
        let mut dialog = GraduateDialog::new(
            "alpha",
            "/tmp/projects/alpha project",
            "$LAB_PROJECTS",
            "/tmp/projects",
        );
        dialog.set_error("Destination cannot be empty");

        dialog.move_to_start();
        dialog.move_forward();
        dialog.kill_to_end();
        assert_eq!(dialog.input, "/");
        assert_eq!(dialog.cursor_pos, 1);
        assert_eq!(dialog.error, None);

        for character in "tmp/test path".chars() {
            dialog.insert_char(character);
        }
        dialog.delete_word_backward();
        assert_eq!(dialog.input, "/tmp/test ");
        assert_eq!(dialog.cursor_pos, 10);

        dialog.backspace();
        assert_eq!(dialog.input, "/tmp/test");
        assert_eq!(dialog.cursor_pos, 9);
    }
}
