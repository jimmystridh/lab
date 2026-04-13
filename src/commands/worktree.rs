//! Worktree command: create a git worktree in a date-prefixed directory.
//!
//! Usage: `lab worktree <repo> [name]`
//!
//! If `repo` is `"dir"` or omitted, uses the current working directory.
//! Generates a shell script that creates a new worktree under labs_path
//! with a YYYY-MM-DD prefix and optionally runs `git worktree add --detach`.
//! For non-git directories, falls back to `mkdir -p` + `cd`.

use crate::script;
use crate::util;
use chrono::Local;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Compute the full worktree target path from a repo directory and optional custom name.
///
/// If `custom_name` is provided and non-empty, it becomes the base name
/// (with spaces replaced by dashes). Otherwise the repo directory's basename
/// is used.
///
/// The base name is then checked for collisions via `resolve_unique_name`
/// and prefixed with today's date.
fn worktree_path(labs_path: &str, repo_dir: &str, custom_name: Option<&str>) -> String {
    let base = if let Some(name) = custom_name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            trimmed.replace(char::is_whitespace, "-")
        } else {
            basename_from_path(repo_dir)
        }
    } else {
        basename_from_path(repo_dir)
    };

    let date_prefix = Local::now().format("%Y-%m-%d").to_string();
    let resolved = util::resolve_unique_name(labs_path, &date_prefix, &base);
    let dir_name = format!("{}-{}", date_prefix, resolved);

    Path::new(labs_path)
        .join(&dir_name)
        .to_string_lossy()
        .to_string()
}

/// Extract basename from a path, resolving to realpath first if possible.
fn basename_from_path(path: &str) -> String {
    // Try to resolve the real path
    if let Ok(real) = fs::canonicalize(path) {
        real.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| Path::new(path).file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "lab".to_string()))
    } else {
        Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "lab".to_string())
    }
}

/// Execute the worktree command.
///
/// Resolves the repo directory, computes the target path,
/// and emits either a worktree script (for git repos) or a
/// mkdir script (for non-git directories).
///
/// # Arguments
/// * `args` - Positional arguments: [repo] [name]
/// * `labs_path` - The labs root directory path
///
/// # Returns
/// Exit code: 0 on success
pub fn cmd_worktree(args: &[String], labs_path: &str) -> i32 {
    let repo_arg = args.first().map(|s| s.as_str());
    let name_args: Vec<&str> = args.iter().skip(1).map(|s| s.as_str()).collect();
    let custom_name = if name_args.is_empty() {
        None
    } else {
        Some(name_args.join(" "))
    };

    // Resolve repo directory: if arg is "dir" or not provided, use cwd
    let repo_dir = match repo_arg {
        Some("dir") | None => env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()),
        Some(path) => {
            let expanded = PathBuf::from(path);
            if expanded.is_absolute() {
                expanded.to_string_lossy().to_string()
            } else {
                // Resolve relative to cwd
                env::current_dir()
                    .map(|cwd| cwd.join(&expanded).to_string_lossy().to_string())
                    .unwrap_or_else(|_| expanded.to_string_lossy().to_string())
            }
        }
    };

    let full_path = worktree_path(labs_path, &repo_dir, custom_name.as_deref());

    // Check if the repo directory is actually a git repo before emitting worktree commands.
    // .git can be a directory (regular repos) or a file (worktrees).
    let git_path = Path::new(&repo_dir).join(".git");
    if git_path.exists() {
        // Git repo → worktree script
        // Determine whether to pass repo or not: if it's cwd, pass None
        let cwd = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let repo_for_script = if repo_dir == cwd {
            None
        } else {
            Some(repo_dir.as_str())
        };

        let cmds = script::script_worktree(&full_path, repo_for_script);
        script::emit_script(&cmds);
    } else {
        // Non-git directory → mkdir script (fallback)
        let cmds = script::script_mkdir_cd(&full_path);
        script::emit_script(&cmds);
    }

    0
}

/// Handle the dot shorthand: `lab . [name]` or `lab ./subdir name`.
///
/// - `lab .` without a name → error
/// - `lab . name` in a git repo → worktree script
/// - `lab . name` outside git repo → mkdir script
/// - `lab ./subdir name` → resolve subdir to absolute path
///
/// # Arguments
/// * `dot_arg` - The dot argument (e.g., ".", "./subdir")
/// * `rest_args` - Remaining arguments after the dot
/// * `labs_path` - The labs root directory path
///
/// # Returns
/// Exit code: 0 on success, 1 on error
pub fn cmd_dot(dot_arg: &str, rest_args: &[String], labs_path: &str) -> i32 {
    let custom = rest_args.join(" ");

    // Bare "lab ." requires a name argument
    if dot_arg == "." && custom.trim().is_empty() {
        eprintln!("Error: 'lab .' requires a name argument");
        eprintln!("Usage: lab . <name>");
        return 1;
    }

    // Resolve the dot path to an absolute directory
    let repo_dir = resolve_dot_path(dot_arg);

    let base = if !custom.trim().is_empty() {
        custom.trim().replace(char::is_whitespace, "-")
    } else {
        basename_from_path(&repo_dir)
    };

    let date_prefix = Local::now().format("%Y-%m-%d").to_string();
    let resolved = util::resolve_unique_name(labs_path, &date_prefix, &base);
    let full_path = Path::new(labs_path)
        .join(format!("{}-{}", date_prefix, resolved))
        .to_string_lossy()
        .to_string();

    // Check if the directory has .git (file for worktrees, directory for repos)
    let git_path = Path::new(&repo_dir).join(".git");
    if git_path.exists() {
        // Git repo → worktree script
        let cmds = script::script_worktree(&full_path, Some(&repo_dir));
        script::emit_script(&cmds);
    } else {
        // Non-git → mkdir script
        let cmds = script::script_mkdir_cd(&full_path);
        script::emit_script(&cmds);
    }

    0
}

/// Resolve a dot path (`.`, `./subdir`, `./path/to/dir`) to an absolute path.
fn resolve_dot_path(dot_arg: &str) -> String {
    let expanded = if dot_arg == "." {
        env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    } else {
        // ./subdir or similar - resolve relative to cwd
        let path = PathBuf::from(dot_arg);
        env::current_dir()
            .map(|cwd| {
                let joined = cwd.join(&path);
                // Try to canonicalize, fall back to the joined path
                fs::canonicalize(&joined)
                    .unwrap_or(joined)
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|_| path.to_string_lossy().to_string())
    };
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn cwd_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_basename_from_path_simple() {
        let result = basename_from_path("/some/path/myrepo");
        assert_eq!(result, "myrepo");
    }

    #[test]
    fn test_basename_from_path_trailing_slash() {
        // PathBuf handles trailing slash by ignoring it
        let result = basename_from_path("/some/path/myrepo/");
        // On most systems this returns empty or "myrepo", but PathBuf strips trailing /
        assert!(!result.is_empty());
    }

    #[test]
    fn test_worktree_path_with_custom_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = worktree_path(dir.path().to_str().unwrap(), "/some/repo", Some("feature"));
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert!(path.contains(&format!("{}-feature", today)));
    }

    #[test]
    fn test_worktree_path_without_custom_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = worktree_path(dir.path().to_str().unwrap(), "/some/repo/myrepo", None);
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert!(path.contains(&format!("{}-myrepo", today)));
    }

    #[test]
    fn test_worktree_path_spaces_to_dashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = worktree_path(dir.path().to_str().unwrap(), "/repo", Some("my feature"));
        assert!(path.contains("my-feature"));
        assert!(!path.contains("my feature"));
    }

    #[test]
    fn test_worktree_path_collision_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        // Create a colliding directory
        std::fs::create_dir(dir.path().join(format!("{}-feature1", today))).unwrap();
        let path = worktree_path(dir.path().to_str().unwrap(), "/repo", Some("feature1"));
        assert!(path.contains(&format!("{}-feature2", today)));
    }

    #[test]
    fn test_resolve_dot_path_bare_dot() {
        let _guard = cwd_test_lock().lock().unwrap();
        let result = resolve_dot_path(".");
        let cwd = env::current_dir().unwrap();
        assert_eq!(result, cwd.to_string_lossy().to_string());
    }

    #[test]
    fn test_cmd_dot_missing_name_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let exit_code = cmd_dot(".", &[], dir.path().to_str().unwrap());
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_cmd_worktree_non_git_uses_mkdir() {
        // When invoked from a non-git directory, worktree should fallback to mkdir
        let labs_dir = tempfile::tempdir().unwrap();
        let non_git_dir = tempfile::tempdir().unwrap();
        // The worktree command with a non-git repo should still succeed (exit 0)
        // and use mkdir instead of worktree commands.
        // We just verify it doesn't crash and returns 0.
        let args = vec![
            non_git_dir.path().to_string_lossy().to_string(),
            "testname".to_string(),
        ];
        let exit_code = cmd_worktree(&args, labs_dir.path().to_str().unwrap());
        assert_eq!(exit_code, 0, "Non-git worktree should succeed with mkdir fallback");
    }

    #[test]
    fn test_cmd_dot_non_git_uses_mkdir() {
        let _guard = cwd_test_lock().lock().unwrap();
        // When invoked from a non-git directory, dot should use mkdir
        let labs_dir = tempfile::tempdir().unwrap();
        let non_git_dir = tempfile::tempdir().unwrap();
        let old_dir = env::current_dir().unwrap();
        let _ = env::set_current_dir(non_git_dir.path());
        let exit_code = cmd_dot(".", &["testname".to_string()], labs_dir.path().to_str().unwrap());
        let _ = env::set_current_dir(&old_dir);
        assert_eq!(exit_code, 0, "Non-git dot should succeed with mkdir fallback");
    }
}
