//! CD/TUI selector path for `lab exec`, `lab exec cd`, and `lab <query>`.

use crate::{
    entries, script,
    tui::{
        app::{App, TerminalSize},
        run_tui, RunOptions, TuiOutcome,
    },
};
use std::path::Path;

/// Execute the cd/TUI selector path.
pub fn cmd_cd(
    args: &[String],
    labs_path: &str,
    and_exit: bool,
    and_keys: Option<&str>,
    and_type: Option<&str>,
    _and_confirm: Option<&str>,
) -> i32 {
    let entries = entries::load_entries(Path::new(labs_path));
    let initial_input = initial_input(args, and_type);
    let mut app = App::new(
        labs_path,
        entries,
        (!initial_input.is_empty()).then_some(initial_input.as_str()),
        TerminalSize::detect(),
    );

    let outcome = match run_tui(
        &mut app,
        RunOptions {
            and_exit,
            and_keys,
            use_test_source: and_exit || and_keys.is_some() || and_type.is_some(),
        },
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("lab: {error}");
            return 1;
        }
    };

    emit_outcome(outcome)
}

fn initial_input(args: &[String], and_type: Option<&str>) -> String {
    and_type
        .map(str::to_owned)
        .unwrap_or_else(|| args.join(" "))
}

fn emit_outcome(outcome: TuiOutcome) -> i32 {
    match outcome {
        TuiOutcome::Selected(path) => {
            let commands = script::script_cd(path.to_string_lossy().as_ref());
            script::emit_script(&commands);
            0
        }
        TuiOutcome::Create(path) => {
            let commands = script::script_mkdir_cd(path.to_string_lossy().as_ref());
            script::emit_script(&commands);
            0
        }
        TuiOutcome::Cancelled { emit_message } => {
            if emit_message {
                println!("Cancelled.");
            }
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_cd_and_exit_returns_1() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("test-dir")).expect("mkdir");

        let exit_code = cmd_cd(&[], dir.path().to_str().unwrap(), true, None, None, None);

        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_cmd_cd_escape_returns_1() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("test-dir")).expect("mkdir");

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
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("test-dir")).expect("mkdir");

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
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("test-dir")).expect("mkdir");

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
        let dir = tempfile::tempdir().expect("tempdir");

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
    fn test_cmd_cd_and_type_prefills_input() {
        let dir = tempfile::tempdir().expect("tempdir");

        let exit_code = cmd_cd(
            &[],
            dir.path().to_str().unwrap(),
            false,
            Some("\r"),
            Some("new project"),
            None,
        );

        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_cmd_cd_no_keys_no_exit_returns_1() {
        let dir = tempfile::tempdir().expect("tempdir");

        let exit_code = cmd_cd(&[], dir.path().to_str().unwrap(), false, None, None, None);

        assert_eq!(exit_code, 1);
    }
}
