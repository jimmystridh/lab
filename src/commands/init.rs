//! Init command: outputs the shell wrapper function to stdout.
//!
//! `lab init [path]` detects the user's shell and prints the appropriate
//! function definition. The output is meant to be eval'd by the shell.

use crate::shell::{self, Shell};
use std::env;
use std::path::PathBuf;

/// Run the `lab init` command.
///
/// Detects the current shell, resolves the binary path, and prints the
/// appropriate init snippet to stdout.
///
/// # Arguments
/// * `args` - Remaining positional args (first may be an explicit labs path)
/// * `_labs_path` - The resolved default labs path. Not used directly;
///   we compute the fallback from env or default.
pub fn cmd_init(args: &[String], _labs_path: &str) {
    let binary_path = resolve_binary_path();

    // If first arg starts with '/', treat it as an explicit path
    let explicit_path = args.first().and_then(|a| {
        if a.starts_with('/') {
            Some(a.as_str())
        } else {
            None
        }
    });

    // The default path for the snippet's env var fallback.
    // This is the path used when LAB_PATH is not set and no explicit path is given.
    // We use ~/src/labs (not expanded) to keep the snippet portable.
    let default_path = "~/src/labs";

    // Detect shell: check SHELL env for fish, otherwise bash/zsh style
    let shell = detect_shell_for_init();

    let snippet = shell::init_snippet(shell, &binary_path, default_path, explicit_path);
    print!("{}", snippet);
}

/// Detect shell specifically for init command.
///
/// Uses a simpler heuristic than the full detect_shell:
/// if SHELL contains "fish", use Fish; otherwise use Bash (bash/zsh use same syntax).
fn detect_shell_for_init() -> Shell {
    if let Ok(shell_env) = env::var("SHELL") {
        if shell_env.contains("fish") {
            return Shell::Fish;
        }
    }

    // For init, default to Bash (bash/zsh produce identical output)
    Shell::Bash
}

/// Resolve the absolute path to the lab binary.
fn resolve_binary_path() -> String {
    // Try to get the real path of the current executable
    if let Ok(exe) = env::current_exe() {
        if let Ok(canonical) = exe.canonicalize() {
            return canonical.to_string_lossy().to_string();
        }
        return exe.to_string_lossy().to_string();
    }

    // Fallback: use argv[0] and try to resolve it
    let arg0 = env::args().next().unwrap_or_else(|| "lab".to_string());
    if arg0.starts_with('/') {
        return arg0;
    }

    // Try to resolve relative path
    if let Ok(cwd) = env::current_dir() {
        let full = cwd.join(&arg0);
        if full.exists() {
            return full.to_string_lossy().to_string();
        }
    }

    // Last resort: search PATH
    if let Ok(path_var) = env::var("PATH") {
        for dir in path_var.split(':') {
            let candidate = PathBuf::from(dir).join(&arg0);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    arg0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_binary_path_returns_something() {
        let path = resolve_binary_path();
        assert!(!path.is_empty(), "binary path should not be empty");
    }

    #[test]
    fn test_detect_shell_for_init_returns_valid() {
        let shell = detect_shell_for_init();
        // Should return either Bash or Fish depending on env
        assert!(
            shell == Shell::Bash || shell == Shell::Fish,
            "init shell should be Bash or Fish"
        );
    }
}
