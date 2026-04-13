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
    if app.is_renaming() {
        return handle_rename_key(app, key);
    }

    if app.is_graduating() {
        return handle_graduate_key(app, key);
    }

    if app.is_confirming_delete() {
        return handle_delete_confirmation_key(app, key);
    }

    match key.code {
        KeyCode::Enter => {
            if app.is_delete_mode() {
                app.begin_delete_confirmation();
                None
            } else {
                Some(selection_outcome(app.current_selection()))
            }
        }
        KeyCode::Esc => {
            if app.is_delete_mode() {
                app.clear_delete_marks();
                None
            } else {
                Some(TuiOutcome::Cancelled)
            }
        }
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

fn handle_rename_key(app: &mut App, key: KeyEvent) -> Option<TuiOutcome> {
    match key.code {
        KeyCode::Enter => match app.submit_rename() {
            Ok(Some(selection)) => Some(TuiOutcome::Rename(selection)),
            Ok(None) | Err(_) => None,
        },
        KeyCode::Esc => {
            app.cancel_rename();
            None
        }
        KeyCode::Backspace => {
            app.backspace();
            None
        }
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_rename_control_key(app, character)
        }
        KeyCode::Char(character) => {
            app.insert_char(character);
            None
        }
        _ => None,
    }
}

fn handle_graduate_key(app: &mut App, key: KeyEvent) -> Option<TuiOutcome> {
    match key.code {
        KeyCode::Enter => match app.submit_graduate() {
            Ok(Some(selection)) => Some(TuiOutcome::Graduate(selection)),
            Ok(None) | Err(_) => None,
        },
        KeyCode::Esc => {
            app.cancel_graduate();
            None
        }
        KeyCode::Backspace => {
            app.backspace();
            None
        }
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_graduate_control_key(app, character)
        }
        KeyCode::Char(character) => {
            app.insert_char(character);
            None
        }
        _ => None,
    }
}

fn handle_delete_confirmation_key(app: &mut App, key: KeyEvent) -> Option<TuiOutcome> {
    match key.code {
        KeyCode::Enter => app.submit_delete_confirmation().map(TuiOutcome::Delete),
        KeyCode::Esc => {
            app.clear_delete_marks();
            None
        }
        KeyCode::Backspace => {
            app.backspace();
            None
        }
        KeyCode::Left => {
            app.move_input_back();
            None
        }
        KeyCode::Right => {
            app.move_input_forward();
            None
        }
        KeyCode::Home => {
            app.move_input_to_start();
            None
        }
        KeyCode::End => {
            app.move_input_to_end();
            None
        }
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_delete_confirmation_control_key(app, character)
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
        'c' => {
            if app.is_delete_mode() {
                app.clear_delete_marks();
                None
            } else {
                Some(TuiOutcome::Cancelled)
            }
        }
        'd' => {
            app.toggle_delete_mark();
            None
        }
        'e' => {
            app.move_input_to_end();
            None
        }
        'f' => {
            app.move_input_forward();
            None
        }
        'g' => {
            app.begin_graduate();
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
        'r' => {
            app.begin_rename();
            None
        }
        't' => app
            .create_new_name()
            .map(|name| TuiOutcome::Mkdir(app.labs_path.join(name))),
        'w' => {
            app.delete_word_backward();
            None
        }
        _ => None,
    }
}

fn handle_rename_control_key(app: &mut App, character: char) -> Option<TuiOutcome> {
    match character.to_ascii_lowercase() {
        'a' => {
            app.move_input_to_start();
            None
        }
        'b' => {
            app.move_input_back();
            None
        }
        'c' => {
            app.cancel_rename();
            None
        }
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
        'k' => {
            app.kill_to_end();
            None
        }
        'w' => {
            app.delete_word_backward();
            None
        }
        _ => None,
    }
}

fn handle_graduate_control_key(app: &mut App, character: char) -> Option<TuiOutcome> {
    match character.to_ascii_lowercase() {
        'a' => {
            app.move_input_to_start();
            None
        }
        'b' => {
            app.move_input_back();
            None
        }
        'c' => {
            app.cancel_graduate();
            None
        }
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
        'k' => {
            app.kill_to_end();
            None
        }
        'w' => {
            app.delete_word_backward();
            None
        }
        _ => None,
    }
}

fn handle_delete_confirmation_control_key(app: &mut App, character: char) -> Option<TuiOutcome> {
    match character.to_ascii_lowercase() {
        'a' => {
            app.move_input_to_start();
            None
        }
        'b' => {
            app.move_input_back();
            None
        }
        'c' => {
            app.clear_delete_marks();
            None
        }
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
        'k' => {
            app.kill_to_end();
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
    use std::{fs, path::PathBuf, time::SystemTime};

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

    fn make_delete_ready_app() -> App {
        let dir = tempfile::tempdir().expect("tempdir");
        let labs_path = dir.path().to_path_buf();
        fs::create_dir(labs_path.join("alpha")).expect("mkdir alpha");
        fs::create_dir(labs_path.join("beta")).expect("mkdir beta");
        std::mem::forget(dir);

        App::new(
            &labs_path,
            vec![
                Entry {
                    name: "alpha".to_string(),
                    path: labs_path.join("alpha"),
                    is_symlink: false,
                    mtime: SystemTime::now(),
                    base_score: 2.0,
                },
                Entry {
                    name: "beta".to_string(),
                    path: labs_path.join("beta"),
                    is_symlink: false,
                    mtime: SystemTime::now(),
                    base_score: 1.0,
                },
            ],
            None,
            TerminalSize::new(80, 24),
        )
    }

    fn make_app_with_create_new_selected() -> App {
        let mut app = make_app(Some("alp"));
        assert_eq!(app.filtered.len(), 1);
        app.move_down();
        assert_eq!(app.current_entry_index(), None);
        app
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
    fn test_ctrl_d_toggles_delete_mark_on_current_entry() {
        let mut app = make_app(None);

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert_eq!(app.marks.len(), 1);
        assert!(app.marks.contains(&0));

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(app.marks.is_empty());
    }

    #[test]
    fn test_ctrl_d_on_create_new_row_does_nothing() {
        let mut app = App::new(
            "/tmp/labs",
            Vec::new(),
            Some("new project"),
            TerminalSize::new(80, 24),
        );

        assert_eq!(app.total_items(), 1);
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(app.marks.is_empty());
    }

    #[test]
    fn test_ctrl_d_on_create_new_row_after_real_entries_does_nothing() {
        let mut app = make_app_with_create_new_selected();

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(app.marks.is_empty());
        assert!(!app.is_delete_mode());
    }

    #[test]
    fn test_ctrl_r_opens_rename_dialog_with_prefilled_name() {
        let mut app = make_app(None);
        app.move_down();

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert!(app.is_renaming());
        let dialog = app.rename_dialog.as_ref().expect("rename dialog");
        assert_eq!(dialog.current_name, "2025-11-15-beta");
        assert_eq!(dialog.input, "2025-11-15-beta");
        assert_eq!(dialog.cursor_pos, 15);
    }

    #[test]
    fn test_ctrl_r_on_create_new_row_does_nothing() {
        let mut app = App::new(
            "/tmp/labs",
            Vec::new(),
            Some("new project"),
            TerminalSize::new(80, 24),
        );

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert!(!app.is_renaming());
    }

    #[test]
    fn test_ctrl_r_on_create_new_row_after_real_entries_does_nothing() {
        let mut app = make_app_with_create_new_selected();

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert!(!app.is_renaming());
    }

    #[test]
    fn test_ctrl_g_opens_graduate_dialog_with_prefilled_destination() {
        let mut app = make_app(None);
        app.move_down();

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert!(app.is_graduating());
        let dialog = app.graduate_dialog.as_ref().expect("graduate dialog");
        assert_eq!(dialog.current_name, "2025-11-15-beta");
        assert!(dialog.input.ends_with("/beta"));
    }

    #[test]
    fn test_ctrl_g_on_create_new_row_does_nothing() {
        let mut app = App::new(
            "/tmp/labs",
            Vec::new(),
            Some("new project"),
            TerminalSize::new(80, 24),
        );

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert!(!app.is_graduating());
    }

    #[test]
    fn test_ctrl_g_on_create_new_row_after_real_entries_does_nothing() {
        let mut app = make_app_with_create_new_selected();

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
        );

        assert!(outcome.is_none());
        assert!(!app.is_graduating());
    }

    #[test]
    fn test_ctrl_t_returns_plain_mkdir_outcome_using_current_input() {
        let mut app = make_app(Some("alp"));
        let expected_path = app
            .labs_path
            .join(app.create_new_name().expect("create-new name"));

        let outcome = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
        );

        assert_eq!(outcome, Some(TuiOutcome::Mkdir(expected_path)));
    }

    #[test]
    fn test_rename_enter_with_same_name_closes_dialog_without_outcome() {
        let mut app = make_app(None);
        app.begin_rename();

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(outcome.is_none());
        assert!(!app.is_renaming());
    }

    #[test]
    fn test_rename_escape_and_ctrl_c_cancel_dialog_without_exiting_selector() {
        for key in [
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ] {
            let mut app = make_app(None);
            app.begin_rename();

            let outcome = handle_key(&mut app, key);

            assert!(outcome.is_none());
            assert!(!app.is_renaming());
        }
    }

    #[test]
    fn test_rename_enter_with_changes_returns_rename_outcome() {
        let mut app = make_app(None);
        app.move_down();
        app.begin_rename();
        app.move_input_to_end();
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        )
        .is_none());

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(
            outcome,
            Some(TuiOutcome::Rename(crate::tui::app::RenameSelection {
                base_path: PathBuf::from("/tmp/labs"),
                old_name: "2025-11-15-beta".to_string(),
                new_name: "2025-11-15-betax".to_string(),
            }))
        );
    }

    #[test]
    fn test_rename_validation_error_stays_in_dialog() {
        let mut app = make_app(None);
        app.begin_rename();
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        )
        .is_none());

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(outcome.is_none());
        assert!(app.is_renaming());
        assert_eq!(
            app.rename_dialog
                .as_ref()
                .and_then(|dialog| dialog.error.clone()),
            Some("Name cannot be empty".to_string())
        );
    }

    #[test]
    fn test_graduate_enter_with_valid_destination_returns_graduate_outcome() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry_path = dir.path().join("2025-11-15-beta");
        fs::create_dir(&entry_path).expect("mkdir source");
        let entry = Entry {
            name: "2025-11-15-beta".to_string(),
            path: entry_path.clone(),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score: 1.0,
        };
        let mut app = App::new(dir.path(), vec![entry], None, TerminalSize::new(80, 24));
        app.begin_graduate();
        let destination = dir.path().join("projects").join("graduated-beta");
        fs::create_dir_all(destination.parent().expect("parent")).expect("mkdir parent");
        if let Some(dialog) = app.graduate_dialog.as_mut() {
            dialog.input = destination.to_string_lossy().into_owned();
            dialog.cursor_pos = dialog.input.chars().count();
        }

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(
            outcome,
            Some(TuiOutcome::Graduate(crate::tui::app::GraduateSelection {
                source: entry_path,
                dest: destination,
                basename: "2025-11-15-beta".to_string(),
                base_path: dir.path().to_path_buf(),
            }))
        );
    }

    #[test]
    fn test_graduate_validation_error_stays_in_dialog() {
        let mut app = make_app(None);
        app.begin_graduate();
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        )
        .is_none());

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(outcome.is_none());
        assert!(app.is_graduating());
        assert_eq!(
            app.graduate_dialog
                .as_ref()
                .and_then(|dialog| dialog.error.clone()),
            Some("Destination cannot be empty".to_string())
        );
    }

    #[test]
    fn test_graduate_escape_and_ctrl_c_cancel_dialog_without_exiting_selector() {
        for key in [
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ] {
            let mut app = make_app(None);
            app.begin_graduate();

            let outcome = handle_key(&mut app, key);

            assert!(outcome.is_none());
            assert!(!app.is_graduating());
        }
    }

    #[test]
    fn test_graduate_line_editing_uses_ctrl_bindings() {
        let mut app = make_app(None);
        app.begin_graduate();

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        )
        .is_none());
        for character in "/tmp/new project".chars() {
            assert!(handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            )
            .is_none());
        }
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('~'), KeyModifiers::NONE),
        )
        .is_none());

        let dialog = app.graduate_dialog.as_ref().expect("graduate dialog");
        assert_eq!(dialog.input, "/tmp/new ~");
        assert_eq!(dialog.cursor_pos, 10);
    }

    #[test]
    fn test_rename_line_editing_uses_ctrl_bindings() {
        let mut app = make_app(None);
        app.begin_rename();

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        )
        .is_none());
        for character in "new name".chars() {
            assert!(handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            )
            .is_none());
        }
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        )
        .is_none());

        let dialog = app.rename_dialog.as_ref().expect("rename dialog");
        assert_eq!(dialog.input, "new /");
        assert_eq!(dialog.cursor_pos, 5);
    }

    #[test]
    fn test_escape_in_delete_mode_clears_marks_without_exiting_immediately() {
        let mut app = make_app(None);
        app.toggle_delete_mark();

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(outcome.is_none());
        assert!(app.marks.is_empty());
        assert!(!app.is_delete_mode());
    }

    #[test]
    fn test_enter_in_delete_mode_opens_confirmation_and_yes_returns_delete_outcome() {
        let mut app = make_delete_ready_app();

        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).is_none());
        assert!(app.is_confirming_delete());

        for character in ['Y', 'E', 'S'] {
            assert!(handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            )
            .is_none());
        }

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            outcome,
            Some(TuiOutcome::Delete(crate::tui::app::DeleteSelection {
                base_path: fs::canonicalize(&app.labs_path).expect("base realpath"),
                basenames: vec!["alpha".to_string()],
            }))
        );
    }

    #[test]
    fn test_delete_confirmation_line_editing_uses_ctrl_bindings() {
        let mut app = make_delete_ready_app();
        app.toggle_delete_mark();
        app.begin_delete_confirmation();

        for character in "YEXX".chars() {
            assert!(handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            )
            .is_none());
        }
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        )
        .is_none());
        assert!(handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        )
        .is_none());

        if let Some(dialog) = app.delete_confirmation.as_ref() {
            assert_eq!(dialog.input, "Y");
            assert_eq!(dialog.cursor_pos, 1);
        } else {
            panic!("delete confirmation should remain active");
        }
    }

    #[test]
    fn test_escape_in_delete_confirmation_clears_marks_and_stays_in_selector() {
        let mut app = make_app(None);
        app.toggle_delete_mark();
        app.begin_delete_confirmation();

        let outcome = handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(outcome.is_none());
        assert!(app.marks.is_empty());
        assert!(!app.is_confirming_delete());
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
