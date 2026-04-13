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

    emit_outcome(outcome, Path::new(labs_path))
}

fn initial_input(args: &[String], and_type: Option<&str>) -> String {
    and_type
        .map(str::to_owned)
        .unwrap_or_else(|| args.join(" "))
}

fn emit_outcome(outcome: TuiOutcome, labs_path: &Path) -> i32 {
    if let Some(commands) = commands_for_outcome(&outcome, labs_path) {
        script::emit_script(&commands);
        0
    } else {
        1
    }
}

fn commands_for_outcome(outcome: &TuiOutcome, labs_path: &Path) -> Option<Vec<String>> {
    match outcome {
        TuiOutcome::Selected(path) => {
            let path = path.to_string_lossy().into_owned();
            Some(script::script_cd(&path))
        }
        TuiOutcome::Create(path) => Some(create_commands(path, labs_path)),
        TuiOutcome::Delete(selection) => Some(script::script_delete(
            &selection.base_path.to_string_lossy(),
            &selection.basenames,
        )),
        TuiOutcome::Cancelled => None,
    }
}

fn create_commands(path: &Path, labs_path: &Path) -> Vec<String> {
    let path = path.to_string_lossy().into_owned();

    if labs_path.join(".git").exists() {
        let repo = labs_path.to_string_lossy().into_owned();
        script::script_worktree(&path, Some(&repo))
    } else {
        script::script_mkdir_cd(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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

    #[test]
    fn test_commands_for_cancelled_outcome_return_none() {
        let dir = tempfile::tempdir().expect("tempdir");

        let commands = commands_for_outcome(&TuiOutcome::Cancelled, dir.path());

        assert!(commands.is_none());
    }

    #[test]
    fn test_create_outcome_uses_mkdir_commands_when_labs_path_is_not_git_repo() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("2026-04-13-feature");

        let commands = commands_for_outcome(&TuiOutcome::Create(path.clone()), dir.path())
            .expect("create commands");

        assert_eq!(
            commands[0],
            format!("mkdir -p '{}'", path.to_string_lossy())
        );
        assert!(commands
            .iter()
            .all(|command| !command.contains("worktree add")));
    }

    #[test]
    fn test_create_outcome_uses_worktree_commands_when_labs_path_is_git_repo() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join(".git")).expect("git dir");
        let path = dir.path().join("2026-04-13-feature");

        let commands = commands_for_outcome(&TuiOutcome::Create(path.clone()), dir.path())
            .expect("create commands");

        assert_eq!(
            commands[0],
            format!("mkdir -p '{}'", path.to_string_lossy())
        );
        assert!(commands[1].contains("Using git worktree"));
        assert!(commands[2].contains("worktree add --detach"));
        assert!(commands[2].contains(&format!("git -C '{}'", dir.path().to_string_lossy())));
        assert!(commands[2].contains("\\$repo"));
        assert!(commands[2].contains(&format!(
            "git -C '{}' rev-parse --show-toplevel",
            dir.path().to_string_lossy()
        )));
    }

    #[test]
    fn test_delete_outcome_uses_batch_delete_script() {
        let dir = tempfile::tempdir().expect("tempdir");
        let commands = commands_for_outcome(
            &TuiOutcome::Delete(crate::tui::app::DeleteSelection {
                base_path: dir.path().to_path_buf(),
                basenames: vec!["alpha".to_string(), "beta".to_string()],
            }),
            dir.path(),
        )
        .expect("delete commands");

        assert_eq!(
            commands[0],
            format!("cd '{}'", dir.path().to_string_lossy())
        );
        assert_eq!(commands[1], "test -d 'alpha' && rm -rf 'alpha'");
        assert_eq!(commands[2], "test -d 'beta' && rm -rf 'beta'");
        assert!(
            commands[3].ends_with(&format!("|| cd '{}'", dir.path().to_string_lossy())),
            "expected restore command to fall back to labs path, got {:?}",
            commands[3]
        );
    }
}
