//! CD/TUI selector path for `lab exec`, `lab exec cd`, and `lab <query>`.
//!
//! In milestone 1, this implements a non-interactive entry selector that:
//! - Loads entries from LAB_PATH and applies fuzzy matching
//! - Processes `--and-keys` to simulate user input (Enter → cd script, ESC → cancel)
//! - Processes `--and-exit` to render a single frame to stderr and exit
//! - Handles text input, navigation, and line editing for test key injection
//! - Falls back to a placeholder message when no test flags are set (full TUI in milestone 2)

use crate::entries::{self, Entry};
use crate::fuzzy::{Fuzzy, MatchResult};
use crate::script;
use chrono::Local;
use std::path::Path;

/// The result of a cd selection: either a script to emit or cancellation.
enum CdResult {
    /// User selected an existing directory → emit cd script
    Cd(String),
    /// User selected "create new" → emit mkdir + cd script
    Mkdir(String),
}

/// Input processing state for simulating TUI key injection.
struct InputState {
    /// The search input buffer
    buffer: String,
    /// Cursor position within the buffer (byte offset matching char offset for ASCII)
    cursor: usize,
    /// Selected list position (0-based)
    list_pos: usize,
}

impl InputState {
    fn new(initial: Option<&str>) -> Self {
        let buffer = initial
            .map(|s| s.replace(char::is_whitespace, "-"))
            .unwrap_or_default();
        let cursor = buffer.len();
        Self {
            buffer,
            cursor,
            list_pos: 0,
        }
    }

    /// Insert a character at the current cursor position.
    fn insert_char(&mut self, c: char) {
        // Only allow valid input characters: [a-zA-Z0-9\-\_\. ]
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ' {
            let byte_pos = self.char_to_byte_pos(self.cursor);
            self.buffer.insert(byte_pos, c);
            self.cursor += 1;
            self.list_pos = 0; // Reset selection on input change
        }
    }

    /// Delete the character before the cursor (backspace).
    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let byte_pos = self.char_to_byte_pos(self.cursor);
            self.buffer.remove(byte_pos);
            self.list_pos = 0; // Reset selection on input change
        }
    }

    /// Move cursor to beginning of line (Ctrl-A).
    fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end of line (Ctrl-E).
    fn move_to_end(&mut self) {
        self.cursor = self.char_count();
    }

    /// Move cursor back one character (Ctrl-B).
    fn move_back(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor forward one character (Ctrl-F).
    fn move_forward(&mut self) {
        if self.cursor < self.char_count() {
            self.cursor += 1;
        }
    }

    /// Delete from cursor to end of line (Ctrl-K).
    fn kill_to_end(&mut self) {
        let byte_pos = self.char_to_byte_pos(self.cursor);
        self.buffer.truncate(byte_pos);
        self.list_pos = 0;
    }

    /// Delete word backward (Ctrl-W).
    /// Deletes back to the previous word boundary (non-alphanumeric char or start).
    fn delete_word_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let chars: Vec<char> = self.buffer.chars().collect();
        let mut new_cursor = self.cursor;

        // Skip any trailing non-alphanumeric characters
        while new_cursor > 0 && !chars[new_cursor - 1].is_ascii_alphanumeric() {
            new_cursor -= 1;
        }

        // Delete back through alphanumeric characters
        while new_cursor > 0 && chars[new_cursor - 1].is_ascii_alphanumeric() {
            new_cursor -= 1;
        }

        let start_byte = self.char_to_byte_pos(new_cursor);
        let end_byte = self.char_to_byte_pos(self.cursor);
        self.buffer.replace_range(start_byte..end_byte, "");
        self.cursor = new_cursor;
        self.list_pos = 0;
    }

    /// Delete entire line (Ctrl-U).
    #[allow(dead_code)]
    fn clear_line(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.list_pos = 0;
    }

    /// Navigate down in the list.
    fn move_down(&mut self, max: usize) {
        if self.list_pos < max.saturating_sub(1) {
            self.list_pos += 1;
        }
    }

    /// Navigate up in the list.
    fn move_up(&mut self) {
        if self.list_pos > 0 {
            self.list_pos -= 1;
        }
    }

    /// Get the number of characters in the buffer.
    fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Convert a character position to a byte position.
    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len())
    }
}

/// Parsed key event from --and-keys input.
#[derive(Debug)]
enum KeyEvent {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Up,
    Down,
    CtrlA,
    CtrlB,
    CtrlC,
    CtrlE,
    CtrlF,
    CtrlH,
    CtrlK,
    CtrlN,
    CtrlP,
    CtrlW,
}

/// Parse a --and-keys string into a sequence of key events.
///
/// Supports:
/// - Raw escape sequences: \x1b[A (up), \x1b[B (down), \x1b (ESC)
/// - Raw control characters: \x01 (Ctrl-A), \x03 (Ctrl-C), etc.
/// - Symbolic names: ENTER, ESC, UP, DOWN, CTRL-A, etc. (comma-separated)
/// - Printable characters
fn parse_and_keys(keys_str: &str) -> Vec<KeyEvent> {
    let mut events = Vec::new();
    let bytes = keys_str.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Check if this looks like symbolic format (contains comma or uppercase words)
    // Symbolic format: "ENTER", "DOWN,ENTER", "CTRL-D,ENTER"
    if keys_str.contains(',')
        || (keys_str.len() >= 2
            && keys_str
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '-' || c == ','))
    {
        // Parse as symbolic names
        for token in keys_str.split(',') {
            match token.trim() {
                "ENTER" | "RETURN" | "CR" => events.push(KeyEvent::Enter),
                "ESC" | "ESCAPE" => events.push(KeyEvent::Escape),
                "UP" => events.push(KeyEvent::Up),
                "DOWN" => events.push(KeyEvent::Down),
                "BACKSPACE" | "BS" => events.push(KeyEvent::Backspace),
                "CTRL-A" => events.push(KeyEvent::CtrlA),
                "CTRL-B" => events.push(KeyEvent::CtrlB),
                "CTRL-C" => events.push(KeyEvent::CtrlC),
                "CTRL-E" => events.push(KeyEvent::CtrlE),
                "CTRL-F" => events.push(KeyEvent::CtrlF),
                "CTRL-H" => events.push(KeyEvent::CtrlH),
                "CTRL-K" => events.push(KeyEvent::CtrlK),
                "CTRL-N" => events.push(KeyEvent::CtrlN),
                "CTRL-P" => events.push(KeyEvent::CtrlP),
                "CTRL-W" => events.push(KeyEvent::CtrlW),
                _ => {
                    // Unknown symbolic key, ignore
                }
            }
        }
        return events;
    }

    // Parse as raw bytes / escape sequences
    while i < len {
        match bytes[i] {
            0x01 => {
                events.push(KeyEvent::CtrlA);
                i += 1;
            }
            0x02 => {
                events.push(KeyEvent::CtrlB);
                i += 1;
            }
            0x03 => {
                events.push(KeyEvent::CtrlC);
                i += 1;
            }
            0x05 => {
                events.push(KeyEvent::CtrlE);
                i += 1;
            }
            0x06 => {
                events.push(KeyEvent::CtrlF);
                i += 1;
            }
            0x08 => {
                events.push(KeyEvent::CtrlH);
                i += 1;
            }
            0x0A => {
                // Ctrl-J → Down navigation (vim-style)
                events.push(KeyEvent::Down);
                i += 1;
            }
            0x0B => {
                events.push(KeyEvent::CtrlK);
                i += 1;
            }
            0x0D => {
                events.push(KeyEvent::Enter);
                i += 1;
            }
            0x0E => {
                events.push(KeyEvent::CtrlN);
                i += 1;
            }
            0x10 => {
                events.push(KeyEvent::CtrlP);
                i += 1;
            }
            0x17 => {
                events.push(KeyEvent::CtrlW);
                i += 1;
            }
            0x1B => {
                // ESC sequence
                if i + 2 < len && bytes[i + 1] == b'[' {
                    // CSI sequence
                    match bytes[i + 2] {
                        b'A' => {
                            events.push(KeyEvent::Up);
                            i += 3;
                        }
                        b'B' => {
                            events.push(KeyEvent::Down);
                            i += 3;
                        }
                        _ => {
                            // Unknown CSI, skip the 3 bytes
                            i += 3;
                        }
                    }
                } else {
                    // Bare ESC
                    events.push(KeyEvent::Escape);
                    i += 1;
                }
            }
            0x7F => {
                events.push(KeyEvent::Backspace);
                i += 1;
            }
            b if (0x20..0x7F).contains(&b) => {
                events.push(KeyEvent::Char(b as char));
                i += 1;
            }
            _ => {
                // Skip unknown bytes
                i += 1;
            }
        }
    }

    events
}

/// Format relative timestamp for an entry.
fn format_relative_time(entry: &Entry) -> String {
    let elapsed = std::time::SystemTime::now()
        .duration_since(entry.mtime)
        .unwrap_or_default();
    let secs = elapsed.as_secs();

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 604800 {
        format!("{}d ago", secs / 86400)
    } else {
        format!("{}w ago", secs / 604800)
    }
}

/// Truncate a name to fit within the given width, appending "…" if truncated.
fn truncate_name(name: &str, max_width: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_width {
        return name.to_string();
    }
    if max_width <= 1 {
        return "…".to_string();
    }
    let truncated: String = name.chars().take(max_width - 1).collect();
    format!("{}…", truncated)
}

/// Render a single TUI frame to stderr for --and-exit mode.
///
/// Shows header, entries with metadata, and footer. This is a simplified
/// version of the full TUI that will be implemented in milestone 2.
fn render_frame(
    entries: &[Entry],
    results: &[MatchResult],
    query: &str,
    list_pos: usize,
    labs_path: &str,
) {
    let width: usize = std::env::var("LAB_WIDTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(80);
    let height: usize = std::env::var("LAB_HEIGHT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);

    let no_colors = crate::NO_COLORS.load(std::sync::atomic::Ordering::Relaxed);

    // Header
    let home_icon = "🏠";
    let title = format!("{} {}", home_icon, labs_path);
    eprintln!("{}", title);

    // Separator
    let separator: String = "─".repeat(width);
    eprintln!("{}", separator);

    // Search input
    eprint!("Search: {}", query);
    eprintln!();

    // Body: entries
    let body_height = height.saturating_sub(5).max(3);
    let entry_count = results.len().min(body_height);

    for (i, result) in results.iter().take(entry_count).enumerate() {
        let entry = &entries[result.index];
        let selected = i == list_pos;

        let indicator = if selected { "→" } else { " " };
        let icon = if entry.is_symlink { "🔗" } else { "📁" };
        let time_str = format_relative_time(entry);
        let score_str = format!("{:.1}", result.score);
        let metadata = format!("{}, {}", time_str, score_str);

        // Calculate available width for the name:
        // indicator(1) + space(1) + icon(2 display cols) + space(1) + name + space(2) + metadata
        let prefix_len = 6; // "→ 📁 " is 6 display columns (indicator + space + icon + space)
        let suffix_len = metadata.len() + 2; // 2 spaces before metadata
        let available = width.saturating_sub(prefix_len + suffix_len);

        let display_name = truncate_name(&entry.name, available);

        if no_colors {
            eprintln!(
                "{} {} {}  {}", indicator, icon, display_name, metadata
            );
        } else if selected {
            eprintln!(
                "\x1b[1m{} {} {}  \x1b[2m{}\x1b[0m",
                indicator, icon, display_name, metadata
            );
        } else {
            eprintln!(
                "{} {} {}  \x1b[2m{}\x1b[0m",
                indicator, icon, display_name, metadata
            );
        }
    }

    // If query is non-empty, show create-new option
    if !query.is_empty() {
        let date_prefix = Local::now().format("%Y-%m-%d").to_string();
        let new_name = format!("{}-{}", date_prefix, query.replace(' ', "-"));
        let selected = entry_count == list_pos;
        let indicator = if selected { "→" } else { " " };
        if no_colors {
            eprintln!("{} 📂 [new] {}", indicator, new_name);
        } else if selected {
            eprintln!("\x1b[1m{} 📂 [new] {}\x1b[0m", indicator, new_name);
        } else {
            eprintln!("{} 📂 [new] {}", indicator, new_name);
        }
    }

    // Footer separator
    eprintln!("{}", separator);

    // Footer keybinding hints
    if no_colors {
        eprintln!("Navigate: ↑/↓  Select: Enter  ^R: Rename  ^G: Graduate  ^D: Delete  Esc: Cancel");
    } else {
        eprintln!(
            "\x1b[2mNavigate: ↑/↓  Select: Enter  ^R: Rename  ^G: Graduate  ^D: Delete  Esc: Cancel\x1b[0m"
        );
    }
}

/// Execute the cd/TUI selector path.
///
/// This is the main entry point for the interactive (or test-injected) directory selector.
/// Loads entries, applies fuzzy matching, processes key events, and emits the appropriate
/// shell script.
///
/// # Arguments
/// * `args` - Remaining positional arguments (used as initial query)
/// * `labs_path` - The labs root directory path
/// * `and_exit` - If true, render one frame and exit
/// * `and_keys` - Optional key sequence to inject
/// * `and_type` - Optional initial input text
/// * `_and_confirm` - Optional confirmation text (for delete dialogs, milestone 3)
///
/// # Returns
/// Exit code: 0 on selection, 1 on cancel
pub fn cmd_cd(
    args: &[String],
    labs_path: &str,
    and_exit: bool,
    and_keys: Option<&str>,
    and_type: Option<&str>,
    _and_confirm: Option<&str>,
) -> i32 {
    // Load entries
    let all_entries = entries::load_entries(Path::new(labs_path));

    let height: usize = std::env::var("LAB_HEIGHT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);
    let limit = height.saturating_sub(6).max(3);

    // Determine initial query from args
    let initial_query = args.join(" ");

    // Determine initial input: --and-type overrides args
    let initial_input = and_type.unwrap_or_else(|| {
        if initial_query.is_empty() {
            ""
        } else {
            &initial_query
        }
    });

    let mut state = InputState::new(if initial_input.is_empty() {
        None
    } else {
        Some(initial_input)
    });

    // If --and-exit with no keys: render one frame and exit 1
    if and_exit && and_keys.is_none() {
        let fuzz = Fuzzy::new(&all_entries);
        let results = fuzz.match_entries(&state.buffer, limit);
        render_frame(&all_entries, &results, &state.buffer, state.list_pos, labs_path);
        return 1;
    }

    // If --and-keys is provided, process the keys
    if let Some(keys_str) = and_keys {
        let key_events = parse_and_keys(keys_str);

        for event in key_events {
            match event {
                KeyEvent::Enter => {
                    // Perform fuzzy match with current buffer
                    let fuzz = Fuzzy::new(&all_entries);
                    let results = fuzz.match_entries(&state.buffer, limit);

                    // Count total items (results + optional create-new)
                    let has_create_new = !state.buffer.is_empty();
                    let total = results.len() + if has_create_new { 1 } else { 0 };

                    if total == 0 {
                        // Nothing to select
                        return 1;
                    }

                    let result = if state.list_pos < results.len() {
                        // Selected an existing entry
                        let entry = &all_entries[results[state.list_pos].index];
                        CdResult::Cd(entry.path.to_string_lossy().to_string())
                    } else {
                        // Selected "create new"
                        let date_prefix = Local::now().format("%Y-%m-%d").to_string();
                        let name = state.buffer.replace(' ', "-");
                        let new_path = Path::new(labs_path)
                            .join(format!("{}-{}", date_prefix, name))
                            .to_string_lossy()
                            .to_string();
                        CdResult::Mkdir(new_path)
                    };

                    // If --and-exit is also set, render frame before emitting
                    if and_exit {
                        let fuzz = Fuzzy::new(&all_entries);
                        let results = fuzz.match_entries(&state.buffer, limit);
                        render_frame(
                            &all_entries,
                            &results,
                            &state.buffer,
                            state.list_pos,
                            labs_path,
                        );
                    }

                    return emit_result(&result);
                }
                KeyEvent::Escape | KeyEvent::CtrlC => {
                    if and_exit {
                        let fuzz = Fuzzy::new(&all_entries);
                        let results = fuzz.match_entries(&state.buffer, limit);
                        render_frame(
                            &all_entries,
                            &results,
                            &state.buffer,
                            state.list_pos,
                            labs_path,
                        );
                    }
                    println!("Cancelled.");
                    return 1;
                }
                KeyEvent::Char(c) => {
                    state.insert_char(c);
                }
                KeyEvent::Backspace | KeyEvent::CtrlH => {
                    state.backspace();
                }
                KeyEvent::Up | KeyEvent::CtrlP => {
                    state.move_up();
                }
                KeyEvent::Down | KeyEvent::CtrlN => {
                    // Need to know total items for clamping
                    let fuzz = Fuzzy::new(&all_entries);
                    let results = fuzz.match_entries(&state.buffer, limit);
                    let has_create_new = !state.buffer.is_empty();
                    let total = results.len() + if has_create_new { 1 } else { 0 };
                    state.move_down(total);
                }
                KeyEvent::CtrlA => {
                    state.move_to_start();
                }
                KeyEvent::CtrlB => {
                    state.move_back();
                }
                KeyEvent::CtrlE => {
                    state.move_to_end();
                }
                KeyEvent::CtrlF => {
                    state.move_forward();
                }
                KeyEvent::CtrlK => {
                    state.kill_to_end();
                }
                KeyEvent::CtrlW => {
                    state.delete_word_backward();
                }
            }
        }

        // Keys exhausted without Enter/ESC: auto-cancel (ESC behavior)
        if and_exit {
            let fuzz = Fuzzy::new(&all_entries);
            let results = fuzz.match_entries(&state.buffer, limit);
            render_frame(
                &all_entries,
                &results,
                &state.buffer,
                state.list_pos,
                labs_path,
            );
        }
        println!("Cancelled.");
        return 1;
    }

    // No test flags: full TUI not yet implemented
    // For --and-exit without --and-keys, we already handled it above
    // This is the case where neither --and-exit nor --and-keys is set
    eprintln!("lab: TUI selector not yet implemented (milestone 2)");
    eprintln!("Cancelled.");
    1
}

/// Emit the appropriate shell script for a cd result.
fn emit_result(result: &CdResult) -> i32 {
    match result {
        CdResult::Cd(path) => {
            let cmds = script::script_cd(path);
            script::emit_script(&cmds);
            0
        }
        CdResult::Mkdir(path) => {
            let cmds = script::script_mkdir_cd(path);
            script::emit_script(&cmds);
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_and_keys tests ----

    #[test]
    fn test_parse_raw_enter() {
        let events = parse_and_keys("\r");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::Enter));
    }

    #[test]
    fn test_parse_raw_escape() {
        let events = parse_and_keys("\x1b");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::Escape));
    }

    #[test]
    fn test_parse_raw_up_arrow() {
        let events = parse_and_keys("\x1b[A");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::Up));
    }

    #[test]
    fn test_parse_raw_down_arrow() {
        let events = parse_and_keys("\x1b[B");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::Down));
    }

    #[test]
    fn test_parse_raw_ctrl_c() {
        let events = parse_and_keys("\x03");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::CtrlC));
    }

    #[test]
    fn test_parse_raw_ctrl_a() {
        let events = parse_and_keys("\x01");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::CtrlA));
    }

    #[test]
    fn test_parse_raw_backspace() {
        let events = parse_and_keys("\x7f");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::Backspace));
    }

    #[test]
    fn test_parse_raw_printable_chars() {
        let events = parse_and_keys("abc");
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], KeyEvent::Char('a')));
        assert!(matches!(events[1], KeyEvent::Char('b')));
        assert!(matches!(events[2], KeyEvent::Char('c')));
    }

    #[test]
    fn test_parse_raw_mixed_keys() {
        // "beta" + Enter
        let events = parse_and_keys("beta\r");
        assert_eq!(events.len(), 5);
        assert!(matches!(events[0], KeyEvent::Char('b')));
        assert!(matches!(events[1], KeyEvent::Char('e')));
        assert!(matches!(events[2], KeyEvent::Char('t')));
        assert!(matches!(events[3], KeyEvent::Char('a')));
        assert!(matches!(events[4], KeyEvent::Enter));
    }

    #[test]
    fn test_parse_symbolic_enter() {
        let events = parse_and_keys("ENTER");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::Enter));
    }

    #[test]
    fn test_parse_symbolic_down_enter() {
        let events = parse_and_keys("DOWN,ENTER");
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], KeyEvent::Down));
        assert!(matches!(events[1], KeyEvent::Enter));
    }

    #[test]
    fn test_parse_symbolic_ctrl_d() {
        let events = parse_and_keys("CTRL-D,ENTER");
        assert_eq!(events.len(), 1); // CTRL-D ignored (not mapped), only ENTER
        assert!(matches!(events[0], KeyEvent::Enter));
    }

    #[test]
    fn test_parse_raw_ctrl_j_is_down() {
        let events = parse_and_keys("\x0a");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::Down));
    }

    #[test]
    fn test_parse_raw_ctrl_n() {
        let events = parse_and_keys("\x0e");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::CtrlN));
    }

    #[test]
    fn test_parse_raw_ctrl_p() {
        let events = parse_and_keys("\x10");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::CtrlP));
    }

    #[test]
    fn test_parse_raw_ctrl_w() {
        let events = parse_and_keys("\x17");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::CtrlW));
    }

    #[test]
    fn test_parse_raw_ctrl_k() {
        let events = parse_and_keys("\x0b");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::CtrlK));
    }

    #[test]
    fn test_parse_raw_ctrl_h() {
        let events = parse_and_keys("\x08");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], KeyEvent::CtrlH));
    }

    // ---- InputState tests ----

    #[test]
    fn test_input_state_new_empty() {
        let state = InputState::new(None);
        assert_eq!(state.buffer, "");
        assert_eq!(state.cursor, 0);
        assert_eq!(state.list_pos, 0);
    }

    #[test]
    fn test_input_state_new_with_value() {
        let state = InputState::new(Some("hello"));
        assert_eq!(state.buffer, "hello");
        assert_eq!(state.cursor, 5);
    }

    #[test]
    fn test_input_state_new_spaces_to_dashes() {
        let state = InputState::new(Some("hello world"));
        assert_eq!(state.buffer, "hello-world");
    }

    #[test]
    fn test_insert_char() {
        let mut state = InputState::new(None);
        state.insert_char('a');
        state.insert_char('b');
        assert_eq!(state.buffer, "ab");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn test_insert_char_resets_list_pos() {
        let mut state = InputState::new(None);
        state.list_pos = 3;
        state.insert_char('a');
        assert_eq!(state.list_pos, 0);
    }

    #[test]
    fn test_backspace() {
        let mut state = InputState::new(Some("abc"));
        state.backspace();
        assert_eq!(state.buffer, "ab");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn test_backspace_at_start() {
        let mut state = InputState::new(None);
        state.backspace(); // Should do nothing
        assert_eq!(state.buffer, "");
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn test_move_to_start() {
        let mut state = InputState::new(Some("hello"));
        state.move_to_start();
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn test_move_to_end() {
        let mut state = InputState::new(Some("hello"));
        state.move_to_start();
        state.move_to_end();
        assert_eq!(state.cursor, 5);
    }

    #[test]
    fn test_move_back_and_forward() {
        let mut state = InputState::new(Some("abc"));
        state.move_back();
        assert_eq!(state.cursor, 2);
        state.move_forward();
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn test_kill_to_end() {
        let mut state = InputState::new(Some("hello"));
        state.cursor = 3;
        state.kill_to_end();
        assert_eq!(state.buffer, "hel");
    }

    #[test]
    fn test_delete_word_backward() {
        let mut state = InputState::new(Some("hello-world"));
        state.delete_word_backward();
        assert_eq!(state.buffer, "hello-");
        assert_eq!(state.cursor, 6);
    }

    #[test]
    fn test_delete_word_backward_full() {
        let mut state = InputState::new(Some("hello"));
        state.delete_word_backward();
        assert_eq!(state.buffer, "");
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn test_move_down_up() {
        let mut state = InputState::new(None);
        state.move_down(5);
        assert_eq!(state.list_pos, 1);
        state.move_down(5);
        assert_eq!(state.list_pos, 2);
        state.move_up();
        assert_eq!(state.list_pos, 1);
    }

    #[test]
    fn test_move_down_clamped() {
        let mut state = InputState::new(None);
        for _ in 0..10 {
            state.move_down(3);
        }
        assert_eq!(state.list_pos, 2);
    }

    #[test]
    fn test_move_up_clamped() {
        let mut state = InputState::new(None);
        state.move_up();
        assert_eq!(state.list_pos, 0);
    }

    // ---- format_relative_time tests ----

    #[test]
    fn test_format_relative_time_just_now() {
        let entry = Entry {
            name: "test".to_string(),
            path: std::path::PathBuf::from("/tmp/test"),
            is_symlink: false,
            mtime: std::time::SystemTime::now(),
            base_score: 0.0,
        };
        let s = format_relative_time(&entry);
        assert_eq!(s, "just now");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let entry = Entry {
            name: "test".to_string(),
            path: std::path::PathBuf::from("/tmp/test"),
            is_symlink: false,
            mtime: std::time::SystemTime::now() - std::time::Duration::from_secs(300),
            base_score: 0.0,
        };
        let s = format_relative_time(&entry);
        assert_eq!(s, "5m ago");
    }

    #[test]
    fn test_format_relative_time_hours() {
        let entry = Entry {
            name: "test".to_string(),
            path: std::path::PathBuf::from("/tmp/test"),
            is_symlink: false,
            mtime: std::time::SystemTime::now() - std::time::Duration::from_secs(7200),
            base_score: 0.0,
        };
        let s = format_relative_time(&entry);
        assert_eq!(s, "2h ago");
    }

    #[test]
    fn test_format_relative_time_days() {
        let entry = Entry {
            name: "test".to_string(),
            path: std::path::PathBuf::from("/tmp/test"),
            is_symlink: false,
            mtime: std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 86400),
            base_score: 0.0,
        };
        let s = format_relative_time(&entry);
        assert_eq!(s, "3d ago");
    }

    #[test]
    fn test_format_relative_time_weeks() {
        let entry = Entry {
            name: "test".to_string(),
            path: std::path::PathBuf::from("/tmp/test"),
            is_symlink: false,
            mtime: std::time::SystemTime::now() - std::time::Duration::from_secs(14 * 86400),
            base_score: 0.0,
        };
        let s = format_relative_time(&entry);
        assert_eq!(s, "2w ago");
    }

    // ---- Integration: cmd_cd tests ----

    #[test]
    fn test_cmd_cd_and_exit_returns_1() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("test-dir")).unwrap();
        let exit_code = cmd_cd(
            &[],
            dir.path().to_str().unwrap(),
            true,
            None,
            None,
            None,
        );
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_cmd_cd_escape_returns_1() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("test-dir")).unwrap();
        let exit_code = cmd_cd(
            &[],
            dir.path().to_str().unwrap(),
            false,
            Some("\x1b"),
            None,
            None,
        );
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_cmd_cd_ctrl_c_returns_1() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("test-dir")).unwrap();
        let exit_code = cmd_cd(
            &[],
            dir.path().to_str().unwrap(),
            false,
            Some("\x03"),
            None,
            None,
        );
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_cmd_cd_enter_returns_0_with_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("test-dir")).unwrap();
        let exit_code = cmd_cd(
            &[],
            dir.path().to_str().unwrap(),
            false,
            Some("\r"),
            None,
            None,
        );
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_cmd_cd_empty_dir_enter_on_create_new() {
        let dir = tempfile::tempdir().unwrap();
        // With empty dir but a typed query, enter should create new
        let exit_code = cmd_cd(
            &[],
            dir.path().to_str().unwrap(),
            false,
            Some("newproject\r"),
            None,
            None,
        );
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_cmd_cd_no_keys_no_exit_returns_1() {
        let dir = tempfile::tempdir().unwrap();
        let exit_code = cmd_cd(
            &[],
            dir.path().to_str().unwrap(),
            false,
            None,
            None,
            None,
        );
        assert_eq!(exit_code, 1);
    }
}
