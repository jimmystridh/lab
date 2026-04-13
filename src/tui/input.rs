//! Key handling for the `lab` TUI selector.
//!
//! This module owns the search-input and list-navigation bindings used by the
//! selector. It updates [`App`](super::app::App) state and returns
//! [`TuiOutcome`](super::TuiOutcome) values when a key triggers selection or
//! cancellation.

use super::{
    app::{App, Selection},
    TuiOutcome,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handle a single key event for the selector.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<TuiOutcome> {
    match key.code {
        KeyCode::Enter => Some(selection_outcome(app.current_selection())),
        KeyCode::Esc => Some(TuiOutcome::Cancelled { emit_message: true }),
        KeyCode::Backspace => {
            app.backspace();
            None
        }
        KeyCode::Up => {
            app.move_up();
            None
        }
        KeyCode::Down => {
            app.move_down();
            None
        }
        KeyCode::Home => {
            app.move_to_top();
            None
        }
        KeyCode::End => {
            app.move_to_bottom();
            None
        }
        KeyCode::PageUp => {
            app.page_up();
            None
        }
        KeyCode::PageDown => {
            app.page_down();
            None
        }
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_control_key(app, character)
        }
        KeyCode::Char(character) => {
            app.insert_char(character);
            None
        }
        _ => None,
    }
}

fn handle_control_key(app: &mut App, character: char) -> Option<TuiOutcome> {
    match character.to_ascii_lowercase() {
        'a' => {
            app.move_input_to_start();
            None
        }
        'b' => {
            app.move_input_back();
            None
        }
        'c' => Some(TuiOutcome::Cancelled { emit_message: true }),
        'e' => {
            app.move_input_to_end();
            None
        }
        'f' => {
            app.move_input_forward();
            None
        }
        'j' | 'n' => {
            app.move_down();
            None
        }
        'k' => {
            app.kill_to_end();
            None
        }
        'p' => {
            app.move_up();
            None
        }
        'w' => {
            app.delete_word_backward();
            None
        }
        _ => None,
    }
}

fn selection_outcome(selection: Option<Selection>) -> TuiOutcome {
    match selection {
        Some(Selection::Existing(path)) => TuiOutcome::Selected(path),
        Some(Selection::Create(path)) => TuiOutcome::Create(path),
        None => TuiOutcome::Cancelled {
            emit_message: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entries::Entry, tui::app::TerminalSize};
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

    fn make_app(initial_input: Option<&str>) -> App {
        App::new(
            "/tmp/labs",
            vec![
                make_entry("2025-11-01-alpha", 2.0),
                make_entry("2025-11-15-beta", 1.5),
                make_entry("2025-11-20-gamma", 1.0),
            ],
            initial_input,
            TerminalSize::new(80, 24),
        )
    }

    #[test]
    fn test_printable_chars_insert_at_cursor_and_refilter() {
        let mut app = make_app(Some("bt"));
        app.cursor_pos = 2;
        app.move_input_back();

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        );

        assert!(outcome.is_none());
        assert_eq!(app.input, "bet");
        assert_eq!(app.input_cursor_pos, 2);
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.entries[app.filtered[0].index].name, "2025-11-15-beta");
        assert_eq!(app.filtered[0].positions, vec![11, 12, 13]);
    }

    #[test]
    fn test_disallowed_chars_are_silently_ignored() {
        let mut app = make_app(Some("be"));
        app.cursor_pos = 1;
        let original_filtered = app
            .filtered
            .iter()
            .map(|result| (result.index, result.positions.clone()))
            .collect::<Vec<_>>();

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        );

        assert!(outcome.is_none());
        assert_eq!(app.input, "be");
        assert_eq!(app.input_cursor_pos, 2);
        assert_eq!(app.cursor_pos, 1);
        assert_eq!(
            app.filtered
                .iter()
                .map(|result| (result.index, result.positions.clone()))
                .collect::<Vec<_>>(),
            original_filtered
        );
    }

    #[test]
    fn test_enter_selects_current_filtered_entry() {
        let mut app = make_app(Some("bet"));

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(
            outcome,
            Some(TuiOutcome::Selected(PathBuf::from("/tmp/2025-11-15-beta")))
        );
    }
}
