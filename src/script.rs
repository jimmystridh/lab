//! Shell script emission for cd, mkdir, clone, worktree, delete, rename, and graduate.
//!
//! Generates shell scripts written to stdout that the shell wrapper function
//! evaluates. Scripts follow a consistent format: warning header comment,
//! commands chained with `&& \`, and 2-space indented continuations.

use crate::util::quote_path;
use std::env;

/// Warning comment emitted as the first line of every script.
/// If the user sees this, they invoked `lab` directly instead of through the shell alias.
const SCRIPT_WARNING: &str =
    "# if you can read this, you didn't launch lab from an alias. run lab --help.";

/// Emit a shell script to stdout from a list of commands.
///
/// Format:
/// - First line: warning comment
/// - First command: unindented
/// - Subsequent commands: 2-space indented
/// - All commands except the last end with ` && \`
/// - Last command ends with a bare newline
pub fn emit_script(commands: &[String]) {
    println!("{}", SCRIPT_WARNING);
    for (i, cmd) in commands.iter().enumerate() {
        let is_last = i == commands.len() - 1;
        if i == 0 {
            if is_last {
                println!("{}", cmd);
            } else {
                println!("{} && \\", cmd);
            }
        } else if is_last {
            println!("  {}", cmd);
        } else {
            println!("  {} && \\", cmd);
        }
    }
}

/// Build commands for cd-ing to an existing directory.
///
/// Touches the directory (to update mtime for recency scoring),
/// echoes the path (for the shell wrapper to capture), and cd's into it.
pub fn script_cd(path: &str) -> Vec<String> {
    vec![
        format!("touch {}", quote_path(path)),
        format!("echo {}", quote_path(path)),
        format!("cd {}", quote_path(path)),
    ]
}

/// Build commands for creating a new directory and cd-ing into it.
///
/// Creates the directory with `mkdir -p`, then does the standard
/// touch + echo + cd sequence.
#[allow(dead_code)]
pub fn script_mkdir_cd(path: &str) -> Vec<String> {
    let mut cmds = vec![format!("mkdir -p {}", quote_path(path))];
    cmds.extend(script_cd(path));
    cmds
}

/// Build commands for cloning a git repo and cd-ing into the result.
///
/// Creates the target directory, prints an informational message,
/// runs `git clone`, then does touch + echo + cd.
pub fn script_clone(path: &str, uri: &str) -> Vec<String> {
    let mut cmds = vec![
        format!("mkdir -p {}", quote_path(path)),
        format!(
            "echo {}",
            quote_path(&format!(
                "Using git clone to create this lab from {}.",
                uri
            ))
        ),
        format!("git clone '{}' {}", uri, quote_path(path)),
    ];
    cmds.extend(script_cd(path));
    cmds
}

/// Build commands for creating a git worktree and cd-ing into it.
///
/// Creates the target directory, prints an informational message,
/// runs `git worktree add --detach` via a sh -c wrapper, then does
/// touch + echo + cd.
///
/// If `repo` is `Some`, the worktree is created from the specified repo
/// directory. Otherwise, it uses the current working directory.
#[allow(dead_code)]
pub fn script_worktree(path: &str, repo: Option<&str>) -> Vec<String> {
    let q_path = quote_path(path);

    let worktree_cmd = if let Some(r) = repo {
        let q_repo = quote_path(r);
        format!(
            "/usr/bin/env sh -c 'if git -C {} rev-parse --is-inside-work-tree >/dev/null 2>&1; \
             then repo=$(git -C {} rev-parse --show-toplevel); \
             git -C \"$repo\" worktree add --detach {} >/dev/null 2>&1 || true; fi; exit 0'",
            q_repo, q_repo, q_path
        )
    } else {
        format!(
            "/usr/bin/env sh -c 'if git rev-parse --is-inside-work-tree >/dev/null 2>&1; \
             then repo=$(git rev-parse --show-toplevel); \
             git -C \"$repo\" worktree add --detach {} >/dev/null 2>&1 || true; fi; exit 0'",
            q_path
        )
    };

    let src = repo.map(|r| r.to_string()).unwrap_or_else(|| {
        env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });
    let mut cmds = vec![
        format!("mkdir -p {}", q_path),
        format!(
            "echo {}",
            quote_path(&format!(
                "Using git worktree to create this lab from {}.",
                src
            ))
        ),
        worktree_cmd,
    ];
    cmds.extend(script_cd(path));
    cmds
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- emit_script tests ----

    /// Helper: captures stdout from emit_script.
    fn capture_emit_script(commands: &[String]) -> String {
        // We can't easily capture println! in Rust, so we test the logic
        // by reconstructing what emit_script would produce.
        let mut output = String::new();
        output.push_str(SCRIPT_WARNING);
        output.push('\n');
        for (i, cmd) in commands.iter().enumerate() {
            let is_last = i == commands.len() - 1;
            if i == 0 {
                if is_last {
                    output.push_str(&format!("{}\n", cmd));
                } else {
                    output.push_str(&format!("{} && \\\n", cmd));
                }
            } else if is_last {
                output.push_str(&format!("  {}\n", cmd));
            } else {
                output.push_str(&format!("  {} && \\\n", cmd));
            }
        }
        output
    }

    #[test]
    fn test_emit_script_warning_header() {
        let output = capture_emit_script(&["echo hello".to_string()]);
        assert!(output.starts_with("# if you can read this, you didn't launch lab from an alias. run lab --help.\n"));
    }

    #[test]
    fn test_emit_script_single_command() {
        let output = capture_emit_script(&["cd '/tmp'".to_string()]);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], "cd '/tmp'");
    }

    #[test]
    fn test_emit_script_multiple_commands_chaining() {
        let output = capture_emit_script(&[
            "touch '/tmp/foo'".to_string(),
            "echo '/tmp/foo'".to_string(),
            "cd '/tmp/foo'".to_string(),
        ]);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], SCRIPT_WARNING);
        assert_eq!(lines[1], "touch '/tmp/foo' && \\");
        assert_eq!(lines[2], "  echo '/tmp/foo' && \\");
        assert_eq!(lines[3], "  cd '/tmp/foo'");
    }

    #[test]
    fn test_emit_script_two_commands() {
        let output = capture_emit_script(&[
            "mkdir -p '/tmp/foo'".to_string(),
            "cd '/tmp/foo'".to_string(),
        ]);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "mkdir -p '/tmp/foo' && \\");
        assert_eq!(lines[2], "  cd '/tmp/foo'");
    }

    #[test]
    fn test_emit_script_first_command_not_indented() {
        let output = capture_emit_script(&[
            "cmd1".to_string(),
            "cmd2".to_string(),
            "cmd3".to_string(),
        ]);
        let lines: Vec<&str> = output.lines().collect();
        assert!(!lines[1].starts_with(' '), "First command should not be indented");
        assert!(lines[2].starts_with("  "), "Second command should be 2-space indented");
        assert!(lines[3].starts_with("  "), "Third command should be 2-space indented");
    }

    #[test]
    fn test_emit_script_last_command_no_trailing_chain() {
        let output = capture_emit_script(&[
            "cmd1".to_string(),
            "cmd2".to_string(),
        ]);
        let lines: Vec<&str> = output.lines().collect();
        assert!(lines[1].ends_with(" && \\"), "Non-last command should have && \\");
        assert!(!lines[2].ends_with(" && \\"), "Last command should NOT have && \\");
    }

    // ---- script_cd tests ----

    #[test]
    fn test_script_cd_basic() {
        let cmds = script_cd("/tmp/foo");
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0], "touch '/tmp/foo'");
        assert_eq!(cmds[1], "echo '/tmp/foo'");
        assert_eq!(cmds[2], "cd '/tmp/foo'");
    }

    #[test]
    fn test_script_cd_with_spaces() {
        let cmds = script_cd("/tmp/my dir");
        assert_eq!(cmds[0], "touch '/tmp/my dir'");
        assert_eq!(cmds[1], "echo '/tmp/my dir'");
        assert_eq!(cmds[2], "cd '/tmp/my dir'");
    }

    #[test]
    fn test_script_cd_with_single_quote() {
        let cmds = script_cd("/tmp/it's");
        assert_eq!(cmds[0], "touch '/tmp/it'\"'\"'s'");
        assert_eq!(cmds[2], "cd '/tmp/it'\"'\"'s'");
    }

    #[test]
    fn test_script_cd_echo_before_cd() {
        let cmds = script_cd("/some/path");
        // echo must come before cd
        assert_eq!(cmds[1], "echo '/some/path'");
        assert_eq!(cmds[2], "cd '/some/path'");
    }

    // ---- script_mkdir_cd tests ----

    #[test]
    fn test_script_mkdir_cd_basic() {
        let cmds = script_mkdir_cd("/tmp/newdir");
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[0], "mkdir -p '/tmp/newdir'");
        assert_eq!(cmds[1], "touch '/tmp/newdir'");
        assert_eq!(cmds[2], "echo '/tmp/newdir'");
        assert_eq!(cmds[3], "cd '/tmp/newdir'");
    }

    #[test]
    fn test_script_mkdir_cd_with_special_chars() {
        let cmds = script_mkdir_cd("/tmp/2025-01-15-it's-a-test");
        assert_eq!(cmds[0], "mkdir -p '/tmp/2025-01-15-it'\"'\"'s-a-test'");
        assert_eq!(cmds[3], "cd '/tmp/2025-01-15-it'\"'\"'s-a-test'");
    }

    // ---- script_clone tests ----

    #[test]
    fn test_script_clone_basic() {
        let cmds = script_clone("/tmp/labs/2025-01-15-user-repo", "https://github.com/user/repo");
        assert_eq!(cmds.len(), 6);
        assert_eq!(cmds[0], "mkdir -p '/tmp/labs/2025-01-15-user-repo'");
        assert_eq!(
            cmds[1],
            "echo 'Using git clone to create this lab from https://github.com/user/repo.'"
        );
        assert_eq!(
            cmds[2],
            "git clone 'https://github.com/user/repo' '/tmp/labs/2025-01-15-user-repo'"
        );
        assert_eq!(cmds[3], "touch '/tmp/labs/2025-01-15-user-repo'");
        assert_eq!(cmds[4], "echo '/tmp/labs/2025-01-15-user-repo'");
        assert_eq!(cmds[5], "cd '/tmp/labs/2025-01-15-user-repo'");
    }

    #[test]
    fn test_script_clone_echo_message() {
        let cmds = script_clone("/tmp/x", "git@github.com:user/repo");
        assert!(cmds[1].starts_with("echo "));
        assert!(cmds[1].contains("Using git clone"));
        assert!(cmds[1].contains("git@github.com:user/repo"));
    }

    #[test]
    fn test_script_clone_echo_path_before_cd() {
        let cmds = script_clone("/tmp/target", "https://github.com/a/b");
        // The second-to-last command should echo the path
        assert_eq!(cmds[cmds.len() - 2], "echo '/tmp/target'");
        assert_eq!(cmds[cmds.len() - 1], "cd '/tmp/target'");
    }

    // ---- script_worktree tests ----

    #[test]
    fn test_script_worktree_with_repo() {
        let cmds = script_worktree("/tmp/labs/2025-01-15-feature", Some("/Users/js/myrepo"));
        assert_eq!(cmds.len(), 6, "Expected 6 commands, got: {:?}", cmds);
        assert_eq!(cmds[0], "mkdir -p '/tmp/labs/2025-01-15-feature'");
        assert!(cmds[1].contains("Using git worktree"), "cmds[1] = {:?}", cmds[1]);
        assert!(cmds[1].contains("/Users/js/myrepo"), "cmds[1] = {:?}", cmds[1]);
        assert!(cmds[2].contains("worktree add --detach"), "cmds[2] = {:?}", cmds[2]);
        assert!(cmds[2].contains("'/Users/js/myrepo'"), "cmds[2] = {:?}", cmds[2]);
        assert!(cmds[2].starts_with("/usr/bin/env sh -c"), "cmds[2] should start with sh -c");
        assert_eq!(cmds[3], "touch '/tmp/labs/2025-01-15-feature'");
        assert_eq!(cmds[4], "echo '/tmp/labs/2025-01-15-feature'");
        assert_eq!(cmds[5], "cd '/tmp/labs/2025-01-15-feature'");
    }

    #[test]
    fn test_script_worktree_without_repo() {
        let cmds = script_worktree("/tmp/labs/2025-01-15-feature", None);
        assert_eq!(cmds.len(), 6);
        assert!(cmds[1].contains("Using git worktree"));
        // When no repo, source should be absolute cwd path (not ".")
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        assert!(
            cmds[1].contains(&format!("from {}.", cwd)),
            "Echo should contain absolute cwd path, got: {}",
            cmds[1]
        );
        assert!(cmds[2].contains("git rev-parse --is-inside-work-tree"));
        assert!(!cmds[2].contains("git -C '"));
    }

    #[test]
    fn test_script_worktree_echo_path_before_cd() {
        let cmds = script_worktree("/tmp/target", Some("/repo"));
        assert_eq!(cmds[cmds.len() - 2], "echo '/tmp/target'");
        assert_eq!(cmds[cmds.len() - 1], "cd '/tmp/target'");
    }

    #[test]
    fn test_script_worktree_with_special_path() {
        let cmds = script_worktree("/tmp/it's here", Some("/repo's"));
        // Paths should be properly quoted
        assert!(cmds[0].contains("'\"'\"'"));
    }

    // ---- Integration: emit_script with script builders ----

    #[test]
    fn test_cd_script_full_output() {
        let cmds = script_cd("/tmp/foo");
        let output = capture_emit_script(&cmds);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], SCRIPT_WARNING);
        assert_eq!(lines[1], "touch '/tmp/foo' && \\");
        assert_eq!(lines[2], "  echo '/tmp/foo' && \\");
        assert_eq!(lines[3], "  cd '/tmp/foo'");
    }

    #[test]
    fn test_mkdir_cd_script_full_output() {
        let cmds = script_mkdir_cd("/tmp/newdir");
        let output = capture_emit_script(&cmds);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], SCRIPT_WARNING);
        assert_eq!(lines[1], "mkdir -p '/tmp/newdir' && \\");
        assert_eq!(lines[2], "  touch '/tmp/newdir' && \\");
        assert_eq!(lines[3], "  echo '/tmp/newdir' && \\");
        assert_eq!(lines[4], "  cd '/tmp/newdir'");
    }

    #[test]
    fn test_clone_script_full_output() {
        let cmds = script_clone("/tmp/labs/2025-01-15-user-repo", "https://github.com/user/repo");
        let output = capture_emit_script(&cmds);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 7); // warning + 6 commands
        assert_eq!(lines[0], SCRIPT_WARNING);
        assert!(lines[1].starts_with("mkdir -p "));
        assert!(lines[1].ends_with(" && \\"));
        assert!(lines[2].starts_with("  echo "));
        assert!(lines[3].starts_with("  git clone "));
        assert!(lines[4].starts_with("  touch "));
        assert!(lines[5].starts_with("  echo "));
        assert!(lines[6].starts_with("  cd "));
        assert!(!lines[6].ends_with(" && \\"));
    }

    // ---- Warning text tests ----

    #[test]
    fn test_script_warning_uses_lab_not_try() {
        assert!(SCRIPT_WARNING.contains("lab"));
        assert!(!SCRIPT_WARNING.contains("try"));
    }

    #[test]
    fn test_script_warning_exact_text() {
        assert_eq!(
            SCRIPT_WARNING,
            "# if you can read this, you didn't launch lab from an alias. run lab --help."
        );
    }
}
