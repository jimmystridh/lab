//! Clone command: clone a git repository into a date-prefixed directory.
//!
//! Usage: `lab clone <git-uri> [name]`
//!
//! Generates a shell script that creates a new directory under labs_path
//! with a YYYY-MM-DD prefix, runs `git clone`, and cd's into the result.

use crate::git;
use crate::script;
use chrono::Local;
use std::path::Path;

/// Generate a directory name for the clone target.
///
/// If `custom_name` is provided, returns `YYYY-MM-DD-custom_name`.
/// Otherwise, parses the git URI to extract `user-repo` and returns
/// `YYYY-MM-DD-user-repo`.
///
/// Returns `None` if no custom name is given and the URI cannot be parsed.
fn generate_clone_directory_name(uri: &str, custom_name: Option<&str>) -> Option<String> {
    let date_prefix = Local::now().format("%Y-%m-%d").to_string();

    if let Some(name) = custom_name {
        if !name.is_empty() {
            return Some(format!("{}-{}", date_prefix, name));
        }
    }

    let parsed = git::parse_git_uri(uri)?;
    Some(format!("{}-{}-{}", date_prefix, parsed.user, parsed.repo))
}

/// Execute the clone command.
///
/// Parses the git URI, generates the target directory name,
/// builds and emits the clone script to stdout.
///
/// # Arguments
/// * `uri` - The git URI to clone (first positional arg, may be None)
/// * `custom_name` - Optional custom name override (second positional arg)
/// * `labs_path` - The labs root directory path
///
/// # Returns
/// Exit code: 0 on success, 1 on error
pub fn cmd_clone(uri: Option<&str>, custom_name: Option<&str>, labs_path: &str) -> i32 {
    let uri = match uri {
        Some(u) if !u.is_empty() => u,
        _ => {
            eprintln!("Error: git URI required for clone command");
            eprintln!("Usage: lab clone <git-uri> [name]");
            return 1;
        }
    };

    // Always validate the URI is parseable, even with a custom name.
    // An unparseable URI means git clone will fail, so reject early.
    if git::parse_git_uri(uri).is_none() {
        eprintln!("Error: Unable to parse git URI: {}", uri);
        return 1;
    }

    let dir_name = match generate_clone_directory_name(uri, custom_name) {
        Some(name) => name,
        None => {
            eprintln!("Error: Unable to parse git URI: {}", uri);
            return 1;
        }
    };

    let full_path = Path::new(labs_path).join(&dir_name);
    let full_path_str = full_path.to_string_lossy();

    let cmds = script::script_clone(&full_path_str, uri);
    script::emit_script(&cmds);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- generate_clone_directory_name tests ----

    #[test]
    fn test_generate_name_https_github() {
        let name =
            generate_clone_directory_name("https://github.com/user/repo", None).unwrap();
        // Should match YYYY-MM-DD-user-repo
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-user-repo", today));
    }

    #[test]
    fn test_generate_name_strips_git_suffix() {
        let name =
            generate_clone_directory_name("https://github.com/user/repo.git", None).unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-user-repo", today));
        assert!(!name.contains(".git"));
    }

    #[test]
    fn test_generate_name_ssh_github() {
        let name =
            generate_clone_directory_name("git@github.com:user/repo", None).unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-user-repo", today));
    }

    #[test]
    fn test_generate_name_ssh_with_git_suffix() {
        let name =
            generate_clone_directory_name("git@github.com:user/myrepo.git", None).unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-user-myrepo", today));
    }

    #[test]
    fn test_generate_name_custom_name() {
        let name = generate_clone_directory_name(
            "https://github.com/user/repo",
            Some("myproject"),
        )
        .unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-myproject", today));
    }

    #[test]
    fn test_generate_name_custom_name_overrides_user_repo() {
        let name = generate_clone_directory_name(
            "https://github.com/user/repo",
            Some("custom"),
        )
        .unwrap();
        assert!(name.contains("custom"));
        assert!(!name.contains("user-repo"));
    }

    #[test]
    fn test_generate_name_empty_custom_falls_back() {
        let name =
            generate_clone_directory_name("https://github.com/user/repo", Some("")).unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-user-repo", today));
    }

    #[test]
    fn test_generate_name_invalid_uri_no_custom() {
        let result = generate_clone_directory_name("not-a-valid-uri", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_name_date_prefix_format() {
        let name =
            generate_clone_directory_name("https://github.com/user/repo", None).unwrap();
        // Check date prefix format: YYYY-MM-DD-
        assert!(
            name.chars().nth(4) == Some('-')
                && name.chars().nth(7) == Some('-')
                && name.chars().nth(10) == Some('-'),
            "Name should start with YYYY-MM-DD-: {}",
            name
        );
    }

    #[test]
    fn test_generate_name_gitlab() {
        let name =
            generate_clone_directory_name("https://gitlab.com/user/glrepo", None).unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-user-glrepo", today));
    }

    #[test]
    fn test_generate_name_generic_host() {
        let name =
            generate_clone_directory_name("https://example.com/myuser/myrepo", None).unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(name, format!("{}-myuser-myrepo", today));
    }

    // ---- cmd_clone tests (exit code checks) ----

    #[test]
    fn test_cmd_clone_no_uri() {
        let code = cmd_clone(None, None, "/tmp/labs");
        assert_eq!(code, 1);
    }

    #[test]
    fn test_cmd_clone_empty_uri() {
        let code = cmd_clone(Some(""), None, "/tmp/labs");
        assert_eq!(code, 1);
    }

    #[test]
    fn test_cmd_clone_unparseable_uri() {
        let code = cmd_clone(Some("not-a-valid-uri"), None, "/tmp/labs");
        assert_eq!(code, 1);
    }

    #[test]
    fn test_cmd_clone_valid_uri() {
        // This will print to stdout (script), but we just check exit code
        let code = cmd_clone(Some("https://github.com/user/repo"), None, "/tmp/labs");
        assert_eq!(code, 0);
    }

    #[test]
    fn test_cmd_clone_valid_uri_with_custom_name() {
        let code = cmd_clone(
            Some("https://github.com/user/repo"),
            Some("myproject"),
            "/tmp/labs",
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn test_cmd_clone_ssh_uri() {
        let code = cmd_clone(Some("git@github.com:user/repo"), None, "/tmp/labs");
        assert_eq!(code, 0);
    }

    #[test]
    fn test_cmd_clone_unparseable_uri_with_custom_name() {
        // Even with a custom name, an unparseable URI should fail
        let code = cmd_clone(Some("not-a-valid-uri"), Some("myproject"), "/tmp/labs");
        assert_eq!(code, 1, "Unparseable URI should be rejected even with custom name");
    }
}
