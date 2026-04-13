//! Delete confirmation dialog state and line-editing support.

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

        let chars: Vec<char> = self.input.chars().collect();
        let mut new_cursor = self.cursor_pos;

        while new_cursor > 0 && !chars[new_cursor - 1].is_ascii_alphanumeric() {
            new_cursor -= 1;
        }

        while new_cursor > 0 && chars[new_cursor - 1].is_ascii_alphanumeric() {
            new_cursor -= 1;
        }

        let start_byte = self.char_to_byte_pos(new_cursor);
        let end_byte = self.char_to_byte_pos(self.cursor_pos);
        self.input.replace_range(start_byte..end_byte, "");
        self.cursor_pos = new_cursor;
    }

    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_pos)
            .map(|(index, _)| index)
            .unwrap_or(self.input.len())
    }
}

fn is_printable(character: char) -> bool {
    !character.is_control()
}

#[cfg(test)]
mod tests {
    use super::DeleteConfirmation;

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
}
