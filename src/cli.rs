//! CLI argument definitions and parsing.
//!
//! Defines the top-level command structure for `lab`:
//! commands (init, install, clone, worktree, exec, cd),
//! global flags (--help, --version, --path, --no-colors),
//! and hidden test infrastructure flags (--and-exit, --and-keys, --and-type, --and-confirm).
//!
//! Note: We do NOT use clap's built-in --help/--version because those write to
//! stdout. We need help and version output on stderr with custom exit codes.

use std::env;

/// Parsed CLI arguments
#[derive(Debug)]
#[allow(dead_code)]
pub struct CliArgs {
    /// The subcommand (init, install, clone, worktree, exec, or None for default)
    pub command: Option<Command>,
    /// --path flag value (overrides LAB_PATH)
    pub path: Option<String>,
    /// --no-colors flag
    pub no_colors: bool,
    /// --and-exit flag (test infrastructure)
    pub and_exit: bool,
    /// --and-keys value (test infrastructure)
    pub and_keys: Option<String>,
    /// --and-type value (test infrastructure)
    pub and_type: Option<String>,
    /// --and-confirm value (test infrastructure)
    pub and_confirm: Option<String>,
    /// Remaining positional arguments
    pub args: Vec<String>,
}

/// Known subcommands
#[derive(Debug, PartialEq)]
pub enum Command {
    Init,
    Install,
    Clone,
    Worktree,
    Exec,
}

/// The result of parsing arguments — may be an early exit request
#[derive(Debug)]
pub enum ParseResult {
    /// Normal execution with parsed args
    Run(CliArgs),
    /// Print help to stderr and exit with the given code
    Help(i32),
    /// Print version to stderr and exit 0
    Version,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Format the help text matching the Ruby version's output
pub fn help_text() -> String {
    format!(
        r#"lab v{version} - ephemeral workspace manager

To use lab, add to your shell config:

  # bash/zsh (~/.bashrc or ~/.zshrc)
  eval "$(lab init ~/src/labs)"

  # fish (~/.config/fish/config.fish)
  eval (lab init ~/src/labs | string collect)

Usage:
  lab [query]           Interactive directory selector
  lab clone <url>       Clone repo into dated directory
  lab worktree <name>   Create worktree from current git repo
  lab --help            Show this help

Commands:
  init [path]           Output shell function definition
  clone <url> [name]    Clone git repo into date-prefixed directory
  worktree <name>       Create worktree in dated directory

Examples:
  lab                   Open interactive selector
  lab project           Selector with initial filter
  lab clone https://github.com/user/repo
  lab worktree feature-branch

Manual mode (without alias):
  lab exec [query]      Output shell script to eval

Environment:
  LAB_PATH          Labs directory (default: ~/src/labs)
  LAB_PROJECTS      Graduate destination (default: parent of LAB_PATH)

Keyboard:
  ↑/↓, Ctrl-P/N     Navigate
  Enter              Select / Create new
  Ctrl-R             Rename
  Ctrl-G             Graduate (promote lab to project)
  Ctrl-D             Mark for deletion
  Ctrl-T             Create new lab
  Esc                Cancel
"#,
        version = VERSION
    )
}

/// Format the version string
pub fn version_text() -> String {
    format!("lab {}", VERSION)
}

/// Extract a `--name VALUE` or `--name=VALUE` option from args (last one wins).
/// Returns the value and removes the option (and its value) from the args vec.
fn extract_option_with_value(args: &mut Vec<String>, opt_name: &str) -> Option<String> {
    // Find the last occurrence
    let mut found_idx = None;
    for (i, arg) in args.iter().enumerate().rev() {
        if arg == opt_name || arg.starts_with(&format!("{}=", opt_name)) {
            found_idx = Some(i);
            break;
        }
    }

    let i = found_idx?;
    let arg = args.remove(i);

    if let Some(eq_pos) = arg.find('=') {
        Some(arg[eq_pos + 1..].to_string())
    } else {
        // The value is the next argument
        if i < args.len() {
            Some(args.remove(i))
        } else {
            None
        }
    }
}

/// Extract a boolean flag from args. Returns true if found (and removes it).
fn extract_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(pos) = args.iter().position(|a| a == flag) {
        args.remove(pos);
        true
    } else {
        false
    }
}

/// Parse command-line arguments.
///
/// This does manual parsing to match the Ruby version's behavior exactly:
/// - --help/-h anywhere → help to stderr, exit 0
/// - --version/-v anywhere → version to stderr, exit 0
/// - no args → help to stderr, exit 2
/// - flags can appear before or after the command
pub fn parse_args() -> ParseResult {
    let mut args: Vec<String> = env::args().skip(1).collect();

    // Process color flags early (before anything else)
    let no_colors = extract_flag(&mut args, "--no-colors")
        || extract_flag(&mut args, "--no-expand-tokens");

    // Check for --help/-h anywhere in args
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return ParseResult::Help(0);
    }

    // Check for --version/-v anywhere in args
    if args.iter().any(|a| a == "--version" || a == "-v") {
        return ParseResult::Version;
    }

    // Extract --path option
    let path = extract_option_with_value(&mut args, "--path");

    // Extract test infrastructure flags
    let and_type = extract_option_with_value(&mut args, "--and-type");
    let and_exit = extract_flag(&mut args, "--and-exit");
    let and_keys = extract_option_with_value(&mut args, "--and-keys");
    let and_confirm = extract_option_with_value(&mut args, "--and-confirm");

    // No args at all → help with exit code 2
    if args.is_empty() && path.is_none() && !and_exit && and_keys.is_none() && and_type.is_none()
    {
        return ParseResult::Help(2);
    }

    // Parse command
    let command = if let Some(first) = args.first() {
        match first.as_str() {
            "init" => {
                args.remove(0);
                Some(Command::Init)
            }
            "install" => {
                args.remove(0);
                Some(Command::Install)
            }
            "clone" => {
                args.remove(0);
                Some(Command::Clone)
            }
            "worktree" => {
                args.remove(0);
                Some(Command::Worktree)
            }
            "exec" => {
                args.remove(0);
                Some(Command::Exec)
            }
            _ => None, // Unknown command treated as search query
        }
    } else {
        None
    };

    ParseResult::Run(CliArgs {
        command,
        path,
        no_colors,
        and_exit,
        and_keys,
        and_type,
        and_confirm,
        args,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to parse a given set of args (simulating command-line input)
    fn parse(args: &[&str]) -> ParseResult {
        let mut arg_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        // Process color flags early
        let no_colors = extract_flag(&mut arg_vec, "--no-colors")
            || extract_flag(&mut arg_vec, "--no-expand-tokens");

        // Check for --help/-h
        if arg_vec.iter().any(|a| a == "--help" || a == "-h") {
            return ParseResult::Help(0);
        }

        // Check for --version/-v
        if arg_vec.iter().any(|a| a == "--version" || a == "-v") {
            return ParseResult::Version;
        }

        // Extract --path option
        let path = extract_option_with_value(&mut arg_vec, "--path");

        // Extract test flags
        let and_type = extract_option_with_value(&mut arg_vec, "--and-type");
        let and_exit = extract_flag(&mut arg_vec, "--and-exit");
        let and_keys = extract_option_with_value(&mut arg_vec, "--and-keys");
        let and_confirm = extract_option_with_value(&mut arg_vec, "--and-confirm");

        // No args at all → help with exit code 2
        if arg_vec.is_empty()
            && path.is_none()
            && !and_exit
            && and_keys.is_none()
            && and_type.is_none()
        {
            return ParseResult::Help(2);
        }

        // Parse command
        let command = if let Some(first) = arg_vec.first() {
            match first.as_str() {
                "init" => {
                    arg_vec.remove(0);
                    Some(Command::Init)
                }
                "install" => {
                    arg_vec.remove(0);
                    Some(Command::Install)
                }
                "clone" => {
                    arg_vec.remove(0);
                    Some(Command::Clone)
                }
                "worktree" => {
                    arg_vec.remove(0);
                    Some(Command::Worktree)
                }
                "exec" => {
                    arg_vec.remove(0);
                    Some(Command::Exec)
                }
                _ => None,
            }
        } else {
            None
        };

        ParseResult::Run(CliArgs {
            command,
            path,
            no_colors,
            and_exit,
            and_keys,
            and_type,
            and_confirm,
            args: arg_vec,
        })
    }

    #[test]
    fn test_help_flag() {
        match parse(&["--help"]) {
            ParseResult::Help(0) => {}
            other => panic!("Expected Help(0), got {:?}", other),
        }
    }

    #[test]
    fn test_help_short_flag() {
        match parse(&["-h"]) {
            ParseResult::Help(0) => {}
            other => panic!("Expected Help(0), got {:?}", other),
        }
    }

    #[test]
    fn test_version_flag() {
        match parse(&["--version"]) {
            ParseResult::Version => {}
            other => panic!("Expected Version, got {:?}", other),
        }
    }

    #[test]
    fn test_version_short_flag() {
        match parse(&["-v"]) {
            ParseResult::Version => {}
            other => panic!("Expected Version, got {:?}", other),
        }
    }

    #[test]
    fn test_no_args_gives_help_exit2() {
        match parse(&[]) {
            ParseResult::Help(2) => {}
            other => panic!("Expected Help(2), got {:?}", other),
        }
    }

    #[test]
    fn test_help_anywhere_in_args() {
        // --help should be detected even after a command
        match parse(&["exec", "--help"]) {
            ParseResult::Help(0) => {}
            other => panic!("Expected Help(0), got {:?}", other),
        }
    }

    #[test]
    fn test_version_anywhere_in_args() {
        match parse(&["exec", "--version"]) {
            ParseResult::Version => {}
            other => panic!("Expected Version, got {:?}", other),
        }
    }

    #[test]
    fn test_init_command() {
        match parse(&["init", "/tmp/labs"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(Command::Init));
                assert_eq!(args.args, vec!["/tmp/labs"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_clone_command() {
        match parse(&["clone", "https://github.com/user/repo"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(Command::Clone));
                assert_eq!(args.args, vec!["https://github.com/user/repo"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_exec_command() {
        match parse(&["exec", "cd", "query"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(Command::Exec));
                assert_eq!(args.args, vec!["cd", "query"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_unknown_command_treated_as_query() {
        match parse(&["myquery"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, None);
                assert_eq!(args.args, vec!["myquery"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_path_equals_form() {
        match parse(&["--path=/custom/dir", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.path, Some("/custom/dir".to_string()));
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_path_space_form() {
        match parse(&["--path", "/custom/dir", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.path, Some("/custom/dir".to_string()));
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_no_colors_flag() {
        match parse(&["--no-colors", "exec"]) {
            ParseResult::Run(args) => {
                assert!(args.no_colors);
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_exit_flag() {
        match parse(&["--and-exit", "exec"]) {
            ParseResult::Run(args) => {
                assert!(args.and_exit);
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_keys_option() {
        match parse(&["--and-keys", "DOWN,ENTER", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.and_keys, Some("DOWN,ENTER".to_string()));
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_type_option() {
        match parse(&["--and-type", "hello", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.and_type, Some("hello".to_string()));
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_confirm_option() {
        match parse(&["--and-confirm", "YES", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.and_confirm, Some("YES".to_string()));
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_combined_flags() {
        match parse(&[
            "--no-colors",
            "--path=/tmp/labs",
            "--and-exit",
            "--and-type",
            "beta",
            "exec",
        ]) {
            ParseResult::Run(args) => {
                assert!(args.no_colors);
                assert_eq!(args.path, Some("/tmp/labs".to_string()));
                assert!(args.and_exit);
                assert_eq!(args.and_type, Some("beta".to_string()));
                assert_eq!(args.command, Some(Command::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_help_text_contains_required_content() {
        let help = help_text();
        assert!(
            help.contains("ephemeral workspace manager"),
            "Help text must contain 'ephemeral workspace manager'"
        );
        assert!(
            help.contains("lab"),
            "Help text must reference 'lab'"
        );
        assert!(
            help.contains("LAB_PATH"),
            "Help text must reference LAB_PATH"
        );
        assert!(
            help.contains("LAB_PROJECTS"),
            "Help text must reference LAB_PROJECTS"
        );
    }

    #[test]
    fn test_version_text_format() {
        let version = version_text();
        assert!(version.starts_with("lab "));
        // Should match pattern "lab X.Y.Z"
        let parts: Vec<&str> = version.split(' ').collect();
        assert_eq!(parts.len(), 2);
        let ver_parts: Vec<&str> = parts[1].split('.').collect();
        assert!(ver_parts.len() >= 2, "Version should have at least major.minor");
    }

    #[test]
    fn test_and_exit_alone_is_not_no_args() {
        // --and-exit without a command should still be Run, not Help
        match parse(&["--and-exit"]) {
            ParseResult::Run(args) => {
                assert!(args.and_exit);
            }
            other => panic!("Expected Run with and_exit, got {:?}", other),
        }
    }

    #[test]
    fn test_path_alone_is_not_no_args() {
        // --path without a command should still be Run (the Ruby version
        // would open the TUI), not Help
        // Actually looking at the Ruby source: --path alone with no command
        // goes through the default path which is the TUI selector.
        // But wait - in our parse, if args is empty after extracting --path,
        // command will be None and args will be empty. That's fine - it means
        // "open TUI with no initial query" which is valid.
        // However, we need to make sure this doesn't trigger the "no args" help case.
        match parse(&["--path=/tmp/labs"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.path, Some("/tmp/labs".to_string()));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_worktree_command() {
        match parse(&["worktree", "feature-name"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(Command::Worktree));
                assert_eq!(args.args, vec!["feature-name"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_install_command() {
        match parse(&["install"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(Command::Install));
                assert!(args.args.is_empty());
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }
}
