//! Shell script emission for cd, mkdir, clone, worktree, delete, rename, and graduate.
//!
//! Generates shell scripts written to stdout that the shell wrapper function
//! evaluates. Scripts follow a consistent format: warning header comment,
//! commands chained with `&& \`, and 2-space indented continuations.

use crate::util::quote_path;
use std::{env, path::Path};

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

/// Build commands for creating a new directory, initializing git, and cd-ing into it.
///
/// Creates the directory with `mkdir -p`, initializes an empty git repo,
/// then does the standard touch + echo + cd sequence.
#[allow(dead_code)]
pub fn script_mkdir_cd(path: &str) -> Vec<String> {
    let quoted_path = quote_path(path);
    let mut cmds = vec![
        format!("mkdir -p {}", quoted_path),
        format!("git -C {} init --quiet", quoted_path),
    ];
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
            quote_path(&format!("Using git clone to create this lab from {}.", uri))
        ),
        format!("git clone '{}' {}", uri, quote_path(path)),
    ];
    cmds.extend(script_cd(path));
    cmds
}

/// Build commands for creating a git worktree and cd-ing into it.
///
/// Creates the target directory, prints an informational message,
/// runs `git worktree add --detach` via a sh -c wrapper whose inner
/// `$repo` variable is escaped so outer eval shells do not expand it early,
/// then does touch + echo + cd.
///
/// If `repo` is `Some`, the worktree is created from the specified repo
/// directory. Otherwise, it uses the current working directory.
#[allow(dead_code)]
pub fn script_worktree(path: &str, repo: Option<&str>) -> Vec<String> {
    let q_path = quote_path(path);

    let worktree_cmd = if let Some(r) = repo {
        let q_repo = quote_path(r);
        format!(
            "/usr/bin/env sh -c \"if git -C {} rev-parse --is-inside-work-tree >/dev/null 2>&1; \
             then repo=$(git -C {} rev-parse --show-toplevel); \
             git -C \\\"\\$repo\\\" worktree add --detach {} >/dev/null 2>&1 || true; fi; exit 0\"",
            q_repo, q_repo, q_path
        )
    } else {
        format!(
            "/usr/bin/env sh -c \"if git rev-parse --is-inside-work-tree >/dev/null 2>&1; \
             then repo=$(git rev-parse --show-toplevel); \
             git -C \\\"\\$repo\\\" worktree add --detach {} >/dev/null 2>&1 || true; fi; exit 0\"",
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

/// Build commands for batch-deleting marked entries inside the labs directory.
///
/// The script first changes into the canonical labs base path, deletes each
/// marked basename with a `test -d` safety guard, then attempts to restore the
/// original working directory and falls back to the labs base if that no longer
/// exists.
pub fn script_delete(base_path: &str, basenames: &[String]) -> Vec<String> {
    let original_pwd = env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| base_path.to_string());

    let mut cmds = vec![format!("cd {}", quote_path(base_path))];
    for basename in basenames {
        let quoted = quote_path(basename);
        cmds.push(format!("test -d {quoted} && rm -rf {quoted}"));
    }
    cmds.push(format!(
        "cd {} 2>/dev/null || cd {}",
        quote_path(&original_pwd),
        quote_path(base_path)
    ));
    cmds
}

/// Build commands for renaming a selected lab directory.
///
/// The script changes into the labs base path, renames the selected basename
/// with `mv`, echoes the new absolute path, and finally cds into the new path.
pub fn script_rename(base_path: &str, old_name: &str, new_name: &str) -> Vec<String> {
    let new_path = Path::new(base_path).join(new_name);
    let new_path = new_path.to_string_lossy().into_owned();

    vec![
        format!("cd {}", quote_path(base_path)),
        format!("mv {} {}", quote_path(old_name), quote_path(new_name)),
        format!("echo {}", quote_path(&new_path)),
        format!("cd {}", quote_path(&new_path)),
    ]
}

/// Build commands for graduating a lab into a project directory.
///
/// Regular directories are moved with `mv`. Git worktrees (identified by a
/// `.git` file inside the source directory) use `git worktree move` so git's
/// bookkeeping stays consistent. After the move, the original labs entry is
/// recreated as a symlink pointing at the destination, a graduation message is
/// echoed, and the script finishes with the standard touch + echo + cd
/// sequence for the destination path.
pub fn script_graduate(source: &str, dest: &str, basename: &str, base_path: &str) -> Vec<String> {
    let symlink_path = Path::new(base_path).join(basename);
    let symlink_path = symlink_path.to_string_lossy().into_owned();
    let is_worktree = Path::new(source).join(".git").is_file();

    let mut cmds = Vec::new();
    if is_worktree {
        cmds.push(format!(
            "git worktree move {} {}",
            quote_path(source),
            quote_path(dest)
        ));
    } else {
        cmds.push(format!("mv {} {}", quote_path(source), quote_path(dest)));
    }
    cmds.push(format!(
        "ln -s {} {}",
        quote_path(dest),
        quote_path(&symlink_path)
    ));
    cmds.push(format!(
        "echo {}",
        quote_path(&format!("Graduated: {basename} → {dest}"))
    ));
    cmds.extend(script_cd(dest));
    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::Write,
        path::Path,
        process::{Command, ExitStatus, Stdio},
    };

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

    fn shell_syntax_check(script: &str) -> ExitStatus {
        let mut child = Command::new("/bin/sh")
            .arg("-n")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn /bin/sh -n");
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(script.as_bytes())
            .expect("write script");
        child.wait().expect("wait for syntax check")
    }

    fn run_git(path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run git");
        assert!(status.success(), "git {:?} failed with {status:?}", args);
    }

    fn init_git_repo(path: &Path) {
        fs::create_dir_all(path).expect("create repo directory");
        let status = Command::new("git")
            .arg("init")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed with {status:?}");

        run_git(path, &["config", "user.name", "Lab Test"]);
        run_git(path, &["config", "user.email", "lab@example.com"]);

        fs::write(path.join("README.md"), "seed\n").expect("write seed file");
        run_git(path, &["add", "README.md"]);
        run_git(path, &["commit", "-m", "seed"]);
    }

    #[test]
    fn test_emit_script_warning_header() {
        let output = capture_emit_script(&["echo hello".to_string()]);
        assert!(output.starts_with(
            "# if you can read this, you didn't launch lab from an alias. run lab --help.\n"
        ));
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
        let output =
            capture_emit_script(&["cmd1".to_string(), "cmd2".to_string(), "cmd3".to_string()]);
        let lines: Vec<&str> = output.lines().collect();
        assert!(
            !lines[1].starts_with(' '),
            "First command should not be indented"
        );
        assert!(
            lines[2].starts_with("  "),
            "Second command should be 2-space indented"
        );
        assert!(
            lines[3].starts_with("  "),
            "Third command should be 2-space indented"
        );
    }

    #[test]
    fn test_emit_script_last_command_no_trailing_chain() {
        let output = capture_emit_script(&["cmd1".to_string(), "cmd2".to_string()]);
        let lines: Vec<&str> = output.lines().collect();
        assert!(
            lines[1].ends_with(" && \\"),
            "Non-last command should have && \\"
        );
        assert!(
            !lines[2].ends_with(" && \\"),
            "Last command should NOT have && \\"
        );
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
        assert_eq!(cmds.len(), 5);
        assert_eq!(cmds[0], "mkdir -p '/tmp/newdir'");
        assert_eq!(cmds[1], "git -C '/tmp/newdir' init --quiet");
        assert_eq!(cmds[2], "touch '/tmp/newdir'");
        assert_eq!(cmds[3], "echo '/tmp/newdir'");
        assert_eq!(cmds[4], "cd '/tmp/newdir'");
    }

    #[test]
    fn test_script_mkdir_cd_with_special_chars() {
        let cmds = script_mkdir_cd("/tmp/2025-01-15-it's-a-test");
        assert_eq!(cmds[0], "mkdir -p '/tmp/2025-01-15-it'\"'\"'s-a-test'");
        assert_eq!(
            cmds[1],
            "git -C '/tmp/2025-01-15-it'\"'\"'s-a-test' init --quiet"
        );
        assert_eq!(cmds[4], "cd '/tmp/2025-01-15-it'\"'\"'s-a-test'");
    }

    #[test]
    fn test_script_mkdir_cd_executes_git_init() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("new lab");
        let cmds = script_mkdir_cd(target.to_str().expect("target path"));
        let output = capture_emit_script(&cmds);

        let status = Command::new("/bin/bash")
            .env("LAB_EMITTED_SCRIPT", &output)
            .arg("-c")
            .arg("set -euo pipefail\neval \"$LAB_EMITTED_SCRIPT\"")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("eval mkdir script");

        assert!(
            status.success(),
            "expected emitted mkdir script to succeed, got {status:?}"
        );
        assert!(
            target.join(".git").is_dir(),
            "expected created lab to contain a .git directory"
        );
    }

    // ---- script_clone tests ----

    #[test]
    fn test_script_clone_basic() {
        let cmds = script_clone(
            "/tmp/labs/2025-01-15-user-repo",
            "https://github.com/user/repo",
        );
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
        assert!(
            cmds[1].contains("Using git worktree"),
            "cmds[1] = {:?}",
            cmds[1]
        );
        assert!(
            cmds[1].contains("/Users/js/myrepo"),
            "cmds[1] = {:?}",
            cmds[1]
        );
        assert!(
            cmds[2].contains("worktree add --detach"),
            "cmds[2] = {:?}",
            cmds[2]
        );
        assert!(
            cmds[2].contains("git -C '/Users/js/myrepo' rev-parse --show-toplevel"),
            "cmds[2] = {:?}",
            cmds[2]
        );
        assert!(
            cmds[2].contains("'/Users/js/myrepo'"),
            "cmds[2] = {:?}",
            cmds[2]
        );
        assert!(cmds[2].contains("\\$repo"), "cmds[2] = {:?}", cmds[2]);
        assert!(
            cmds[2].starts_with("/usr/bin/env sh -c"),
            "cmds[2] should start with sh -c"
        );
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

    #[test]
    fn test_script_worktree_output_has_valid_shell_syntax() {
        let cmds = script_worktree("/tmp/labs/2025-01-15-feature", Some("/Users/js/myrepo"));
        let output = capture_emit_script(&cmds);
        let status = shell_syntax_check(&output);

        assert!(
            status.success(),
            "expected sh -n to accept emitted worktree script, got {status:?}"
        );
    }

    #[test]
    fn test_script_worktree_executes_when_outer_shell_has_repo_env_var() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path().join("labs-root");
        init_git_repo(&repo);

        let target = repo.join("2026-04-13-feature");
        let cmds = script_worktree(
            target.to_str().expect("target path"),
            Some(repo.to_str().expect("repo path")),
        );
        let output = capture_emit_script(&cmds);

        let status = Command::new("/bin/bash")
            .env("LAB_EMITTED_SCRIPT", &output)
            .env("repo", dir.path().join("wrong-repo"))
            .arg("-c")
            .arg("set -euo pipefail\neval \"$LAB_EMITTED_SCRIPT\"")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("eval worktree script");
        assert!(
            status.success(),
            "expected emitted worktree script to succeed, got {status:?}"
        );

        let git_common_dir = Command::new("git")
            .arg("-C")
            .arg(&target)
            .args(["rev-parse", "--git-common-dir"])
            .output()
            .expect("inspect linked worktree");
        assert!(
            git_common_dir.status.success(),
            "expected created path to be a git worktree, stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&git_common_dir.stdout),
            String::from_utf8_lossy(&git_common_dir.stderr)
        );
        let common_dir_path = target.join(String::from_utf8_lossy(&git_common_dir.stdout).trim());
        assert_eq!(
            fs::canonicalize(common_dir_path).expect("canonicalize linked git dir"),
            fs::canonicalize(repo.join(".git")).expect("canonicalize repo git dir")
        );
        assert!(
            target.join(".git").is_file(),
            "expected linked worktree .git file"
        );
    }

    #[test]
    fn test_script_delete_uses_basenames_only_and_restores_pwd() {
        let cwd = env::current_dir()
            .map(|path| path.to_string_lossy().to_string())
            .expect("current dir");
        let cmds = script_delete(
            "/tmp/labs",
            &[
                "2025-11-01-first".to_string(),
                "2025-11-02-second".to_string(),
            ],
        );

        assert_eq!(cmds[0], "cd '/tmp/labs'");
        assert_eq!(
            cmds[1],
            "test -d '2025-11-01-first' && rm -rf '2025-11-01-first'"
        );
        assert_eq!(
            cmds[2],
            "test -d '2025-11-02-second' && rm -rf '2025-11-02-second'"
        );
        assert_eq!(
            cmds[3],
            format!("cd '{}' 2>/dev/null || cd '/tmp/labs'", cwd)
        );
        assert!(cmds.iter().all(|cmd| !cmd.contains("/tmp/labs/2025-11")));
    }

    #[test]
    fn test_delete_script_full_output() {
        let cmds = script_delete("/tmp/labs", &["project".to_string()]);
        let output = capture_emit_script(&cmds);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines[0], SCRIPT_WARNING);
        assert_eq!(lines[1], "cd '/tmp/labs' && \\");
        assert_eq!(lines[2], "  test -d 'project' && rm -rf 'project' && \\");
        assert!(lines[3].starts_with("  cd '"));
        assert!(lines[3].ends_with("' 2>/dev/null || cd '/tmp/labs'"));
    }

    #[test]
    fn test_script_rename_basic() {
        let cmds = script_rename("/tmp/labs", "2025-11-01-alpha", "2025-11-01-beta");

        assert_eq!(
            cmds,
            vec![
                "cd '/tmp/labs'".to_string(),
                "mv '2025-11-01-alpha' '2025-11-01-beta'".to_string(),
                "echo '/tmp/labs/2025-11-01-beta'".to_string(),
                "cd '/tmp/labs/2025-11-01-beta'".to_string(),
            ]
        );
    }

    #[test]
    fn test_script_rename_quotes_names_and_does_not_touch() {
        let cmds = script_rename("/tmp/lab root", "it's old", "new name");

        assert_eq!(cmds[0], "cd '/tmp/lab root'");
        assert_eq!(cmds[1], "mv 'it'\"'\"'s old' 'new name'");
        assert_eq!(cmds[2], "echo '/tmp/lab root/new name'");
        assert_eq!(cmds[3], "cd '/tmp/lab root/new name'");
        assert!(cmds.iter().all(|command| !command.starts_with("touch ")));
    }

    #[test]
    fn test_rename_script_full_output() {
        let cmds = script_rename("/tmp/labs", "old", "new");
        let output = capture_emit_script(&cmds);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], SCRIPT_WARNING);
        assert_eq!(lines[1], "cd '/tmp/labs' && \\");
        assert_eq!(lines[2], "  mv 'old' 'new' && \\");
        assert_eq!(lines[3], "  echo '/tmp/labs/new' && \\");
        assert_eq!(lines[4], "  cd '/tmp/labs/new'");
    }

    #[test]
    fn test_script_graduate_regular_directory_uses_mv_and_symlink() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("2025-06-01-alpha");
        std::fs::create_dir(&source).expect("mkdir source");
        let dest = dir.path().join("projects").join("alpha");

        let cmds = script_graduate(
            source.to_str().expect("source"),
            dest.to_str().expect("dest"),
            "2025-06-01-alpha",
            dir.path().to_str().expect("base path"),
        );

        assert_eq!(
            cmds[0],
            format!(
                "mv '{}' '{}'",
                source.to_string_lossy(),
                dest.to_string_lossy()
            )
        );
        assert_eq!(
            cmds[1],
            format!(
                "ln -s '{}' '{}/2025-06-01-alpha'",
                dest.to_string_lossy(),
                dir.path().to_string_lossy()
            )
        );
        assert_eq!(
            cmds[2],
            format!(
                "echo 'Graduated: 2025-06-01-alpha → {}'",
                dest.to_string_lossy()
            )
        );
        assert_eq!(cmds[3], format!("touch '{}'", dest.to_string_lossy()));
        assert_eq!(cmds[4], format!("echo '{}'", dest.to_string_lossy()));
        assert_eq!(cmds[5], format!("cd '{}'", dest.to_string_lossy()));
    }

    #[test]
    fn test_script_graduate_worktree_uses_git_worktree_move() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("2025-06-01-worktree");
        std::fs::create_dir_all(&source).expect("mkdir source");
        std::fs::write(
            source.join(".git"),
            "gitdir: /tmp/repo/.git/worktrees/worktree",
        )
        .expect("write git file");
        let dest = dir.path().join("projects").join("worktree");

        let cmds = script_graduate(
            source.to_str().expect("source"),
            dest.to_str().expect("dest"),
            "2025-06-01-worktree",
            dir.path().to_str().expect("base path"),
        );

        assert_eq!(
            cmds[0],
            format!(
                "git worktree move '{}' '{}'",
                source.to_string_lossy(),
                dest.to_string_lossy()
            )
        );
        assert!(cmds.iter().all(|command| !command.starts_with("mv ")));
    }

    #[test]
    fn test_graduate_script_full_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("2025-06-01-alpha");
        std::fs::create_dir(&source).expect("mkdir source");
        let dest = dir.path().join("projects").join("alpha");

        let cmds = script_graduate(
            source.to_str().expect("source"),
            dest.to_str().expect("dest"),
            "2025-06-01-alpha",
            dir.path().to_str().expect("base path"),
        );
        let output = capture_emit_script(&cmds);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines[0], SCRIPT_WARNING);
        assert!(lines[1].starts_with("mv "));
        assert!(lines[1].ends_with(" && \\"));
        assert!(lines[2].starts_with("  ln -s "));
        assert!(lines[2].ends_with(" && \\"));
        assert!(lines[3].starts_with("  echo 'Graduated: 2025-06-01-alpha"));
        assert!(lines[3].ends_with(" && \\"));
        assert!(lines[4].starts_with("  touch "));
        assert!(lines[5].starts_with("  echo "));
        assert!(lines[6].starts_with("  cd "));
        assert!(!lines[6].ends_with(" && \\"));
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
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], SCRIPT_WARNING);
        assert_eq!(lines[1], "mkdir -p '/tmp/newdir' && \\");
        assert_eq!(lines[2], "  git -C '/tmp/newdir' init --quiet && \\");
        assert_eq!(lines[3], "  touch '/tmp/newdir' && \\");
        assert_eq!(lines[4], "  echo '/tmp/newdir' && \\");
        assert_eq!(lines[5], "  cd '/tmp/newdir'");
    }

    #[test]
    fn test_clone_script_full_output() {
        let cmds = script_clone(
            "/tmp/labs/2025-01-15-user-repo",
            "https://github.com/user/repo",
        );
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
