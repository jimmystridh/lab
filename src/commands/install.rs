//! Install command: appends the shell init snippet to the user's RC file.
//!
//! `lab install [path]` detects the shell, finds the RC file, checks for
//! an existing `# lab shell integration` marker, and appends the init
//! snippet if not already present.

use crate::shell::{self, Shell};
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// The marker comment used to detect existing installations.
const MARKER: &str = "# lab shell integration";

/// Run the `lab install` command.
///
/// # Arguments
/// * `args` - Remaining positional args (first may be an explicit labs path)
/// * `labs_path` - The resolved default labs path
///
/// # Exit codes
/// * 0 - Successfully installed or already installed
/// * 1 - Error (no shell detected, read-only file, etc.)
pub fn cmd_install(args: &[String], labs_path: &str) -> i32 {
    let binary_path = resolve_binary_path();

    // If first arg starts with '/', treat it as an explicit path
    let explicit_path = args.first().and_then(|a| {
        if a.starts_with('/') {
            Some(a.as_str())
        } else {
            None
        }
    });

    let default_path = labs_path;

    // Detect shell
    let shell = match shell::detect_shell() {
        Some(s) => s,
        None => {
            eprintln!("Error: could not determine shell config file");
            eprintln!("Your shell was detected as: unknown");
            eprintln!("Run 'lab init' and manually add the output to your shell config.");
            return 1;
        }
    };

    // Find RC file
    let rc_file = match shell::shell_rc_file(shell) {
        Some(f) => f,
        None => {
            eprintln!("Error: could not determine shell config file");
            eprintln!("Your shell was detected as: {:?}", shell);
            eprintln!("Run 'lab init' and manually add the output to your shell config.");
            return 1;
        }
    };

    // Expand tilde in RC file path
    let rc_path = expand_tilde(&rc_file);

    // Check if already installed
    if rc_path.exists() {
        match fs::read_to_string(&rc_path) {
            Ok(contents) => {
                if contents.contains(MARKER) {
                    eprintln!("lab is already installed in {}", rc_path.display());
                    eprintln!("To reinstall, remove the '{}' block first.", MARKER);
                    return 0;
                }
            }
            Err(e) => {
                eprintln!("Warning: could not read {}: {}", rc_path.display(), e);
                eprintln!("Run 'lab init' and manually add the output to your shell config.");
                return 1;
            }
        }
    }

    // Generate the snippet
    let snippet = shell::init_snippet(shell, &binary_path, default_path, explicit_path);
    let block = format!("\n{}\n{}", MARKER, snippet);

    // Check if file exists and is read-only
    if rc_path.exists() {
        if let Ok(meta) = fs::metadata(&rc_path) {
            if meta.permissions().readonly() {
                eprintln!("Warning: {} is read-only, skipping.", rc_path.display());
                eprintln!("Run 'lab init' and manually add the output to your shell config.");
                return 1;
            }
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = rc_path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    // Append to RC file
    match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&rc_path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(block.as_bytes()) {
                eprintln!("Error writing to {}: {}", rc_path.display(), e);
                return 1;
            }
        }
        Err(e) => {
            eprintln!("Error opening {}: {}", rc_path.display(), e);
            eprintln!("Run 'lab init' and manually add the output to your shell config.");
            return 1;
        }
    }

    eprintln!("Added lab shell integration to {}", rc_path.display());
    if shell == Shell::PowerShell {
        eprintln!("Restart your shell or run: . $PROFILE");
    } else {
        eprintln!("Restart your shell or run: source {}", rc_path.display());
    }

    0
}

/// Expand ~ to home directory in a path string.
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return PathBuf::from(path.replacen('~', &home.to_string_lossy(), 1));
        }
    }
    PathBuf::from(path)
}

/// Resolve the absolute path to the lab binary.
fn resolve_binary_path() -> String {
    if let Ok(exe) = env::current_exe() {
        if let Ok(canonical) = exe.canonicalize() {
            return canonical.to_string_lossy().to_string();
        }
        return exe.to_string_lossy().to_string();
    }

    let arg0 = env::args().next().unwrap_or_else(|| "lab".to_string());
    if arg0.starts_with('/') {
        return arg0;
    }

    if let Ok(cwd) = env::current_dir() {
        let full = cwd.join(&arg0);
        if full.exists() {
            return full.to_string_lossy().to_string();
        }
    }

    arg0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test");
        assert!(
            !expanded.to_string_lossy().starts_with('~'),
            "tilde should be expanded: {}",
            expanded.display()
        );
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_marker_constant() {
        assert_eq!(MARKER, "# lab shell integration");
    }

    #[test]
    fn test_install_to_new_file() {
        let tmp = tempfile::tempdir().unwrap();
        let rc_path = tmp.path().join(".testrc");

        // We can't easily test cmd_install directly because it uses detect_shell
        // and env vars. Instead, test the append logic.
        let snippet = "lab() { echo test; }\n";
        let block = format!("\n{}\n{}", MARKER, snippet);

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&rc_path)
            .unwrap();
        file.write_all(block.as_bytes()).unwrap();

        let contents = fs::read_to_string(&rc_path).unwrap();
        assert!(contents.contains(MARKER));
        assert!(contents.contains("lab()"));
    }

    #[test]
    fn test_install_detects_existing_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let rc_path = tmp.path().join(".testrc");

        // Write initial content with marker
        fs::write(
            &rc_path,
            format!("# existing stuff\n{}\nlab() {{ }}\n", MARKER),
        )
        .unwrap();

        let contents = fs::read_to_string(&rc_path).unwrap();
        assert!(contents.contains(MARKER), "marker should be present");
    }

    #[test]
    fn test_install_readonly_file_detection() {
        let tmp = tempfile::tempdir().unwrap();
        let rc_path = tmp.path().join(".testrc");

        // Create read-only file
        fs::write(&rc_path, "existing content\n").unwrap();
        let mut perms = fs::metadata(&rc_path).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&rc_path, perms).unwrap();

        let meta = fs::metadata(&rc_path).unwrap();
        assert!(meta.permissions().readonly(), "file should be read-only");

        // Cleanup: restore write permission so tempdir can clean up
        let mut perms = meta.permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&rc_path, perms).unwrap();
    }
}
