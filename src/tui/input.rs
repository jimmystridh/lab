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
        KeyCode::Esc => Some(TuiOutcome::Cancelled),
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
        'c' => Some(TuiOutcome::Cancelled),
        'e' => {
            app.move_input_to_end();
            None
        }
        'f' => {
            app.move_input_forward();
            None
        }
        'h' => {
            app.backspace();
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
        None => TuiOutcome::Cancelled,
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

    #[test]
    fn test_escape_cancels_without_stdout_message_flag() {
        let mut app = make_app(None);

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(outcome, Some(TuiOutcome::Cancelled));
    }

    #[test]
    fn test_ctrl_c_cancels_without_stdout_message_flag() {
        let mut app = make_app(None);

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );

        assert_eq!(outcome, Some(TuiOutcome::Cancelled));
    }

    #[test]
    fn test_down_bindings_move_selection_down() {
        let down_keys = [
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        ];

        for key in down_keys {
            let mut app = make_app(None);
            let outcome = handle_key(&mut app, key);

            assert!(outcome.is_none());
            assert_eq!(app.cursor_pos, 1);
        }
    }

    #[test]
    fn test_up_bindings_move_selection_up() {
        let up_keys = [
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        ];

        for key in up_keys {
            let mut app = make_app(None);
            app.move_down();
            app.move_down();

            let outcome = handle_key(&mut app, key);

            assert!(outcome.is_none());
            assert_eq!(app.cursor_pos, 1);
        }
    }

    #[test]
    fn test_home_end_and_page_keys_use_navigation_methods() {
        let mut app = make_scrolling_app(8, 8);

        assert!(handle_key(&mut app, KeyEvent::new(KeyCode::End, KeyModifiers::NONE)).is_none());
        assert_eq!(app.cursor_pos, app.total_items() - 1);
        assert_eq!(app.scroll_offset, 5);

        assert!(handle_key(&mut app, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)).is_none());
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)
        )
        .is_none());
        assert_eq!(app.cursor_pos, app.visible_result_limit());
        assert_eq!(app.scroll_offset, 1);

        assert!(handle_key(&mut app, KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)).is_none());
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_ctrl_a_e_b_and_f_move_input_cursor_within_bounds() {
        let mut app = make_app(Some("beta"));

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert_eq!(app.input_cursor_pos, 0);

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert_eq!(app.input_cursor_pos, 0);

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert_eq!(app.input_cursor_pos, 1);

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert_eq!(app.input_cursor_pos, 4);

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert_eq!(app.input_cursor_pos, 4);
    }

    #[test]
    fn test_backspace_re_evaluates_matches_after_deleting_before_cursor() {
        let mut app = make_app(Some("betaa"));
        app.cursor_pos = 2;
        app.scroll_offset = 1;

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        );

        assert!(outcome.is_none());
        assert_eq!(app.input, "beta");
        assert_eq!(app.input_cursor_pos, 4);
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.entries[app.filtered[0].index].name, "2025-11-15-beta");
    }

    #[test]
    fn test_ctrl_k_kills_to_end_and_re_evaluates_matches() {
        let mut app = make_app(Some("alphabeta"));
        app.cursor_pos = 2;
        app.scroll_offset = 1;
        app.input_cursor_pos = 5;

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert_eq!(app.input, "alpha");
        assert_eq!(app.input_cursor_pos, 5);
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.entries[app.filtered[0].index].name, "2025-11-01-alpha");
    }

    #[test]
    fn test_ctrl_w_deletes_previous_word_but_keeps_boundary_characters() {
        let mut app = make_app(Some("hello-world"));
        app.cursor_pos = 2;
        app.scroll_offset = 1;

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert_eq!(app.input, "hello-");
        assert_eq!(app.input_cursor_pos, 6);
        assert_eq!(app.cursor_pos, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_ctrl_k_without_buffer_change_keeps_selection_position() {
        let mut app = make_app(Some("beta"));
        app.cursor_pos = 2;
        app.move_input_to_end();

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert_eq!(app.input, "beta");
        assert_eq!(app.cursor_pos, 2);
    }

    #[test]
    fn test_ctrl_h_aliases_backspace_behavior() {
        let mut app = make_app(Some("betaa"));

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert_eq!(app.input, "beta");
        assert_eq!(app.input_cursor_pos, 4);
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.entries[app.filtered[0].index].name, "2025-11-15-beta");
    }
}
