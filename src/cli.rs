//! CLI argument definitions and parsing using clap derive macros.
//!
//! Defines the top-level command structure for `lab`:
//! commands (init, install, clone, worktree, exec, cd),
//! global flags (--help, --version, --path, --no-colors),
//! and hidden test infrastructure flags (--and-exit, --and-keys, --and-type, --and-confirm).
//!
//! Note: We do NOT use clap's built-in --help/--version because those write to
//! stdout. We need help and version output on stderr with custom exit codes.

use clap::{Parser, Subcommand};
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// lab - ephemeral workspace manager
#[derive(Parser, Debug)]
#[command(
    name = "lab",
    disable_help_flag = true,
    disable_version_flag = true,
    allow_external_subcommands = true
)]
pub struct Cli {
    /// Override labs directory (default: ~/src/labs)
    #[arg(long, global = true)]
    pub path: Option<String>,

    /// Disable ANSI color codes in output
    #[arg(long = "no-colors", global = true)]
    pub no_colors: bool,

    /// Alias for --no-colors (test infrastructure)
    #[arg(long = "no-expand-tokens", global = true, hide = true)]
    pub no_expand_tokens: bool,

    /// Show help text
    #[arg(long, short = 'h', global = true)]
    pub help: bool,

    /// Show version number
    #[arg(long, short = 'v', global = true)]
    pub version: bool,

    /// Render TUI once and exit (test infrastructure)
    #[arg(long = "and-exit", global = true, hide = true)]
    pub and_exit: bool,

    /// Inject key sequence (test infrastructure)
    #[arg(long = "and-keys", global = true, hide = true)]
    pub and_keys: Option<String>,

    /// Set initial input buffer (test infrastructure)
    #[arg(long = "and-type", global = true, hide = true)]
    pub and_type: Option<String>,

    /// Inject confirmation text (test infrastructure)
    #[arg(long = "and-confirm", global = true, hide = true)]
    pub and_confirm: Option<String>,

    /// The subcommand to run
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Known subcommands
#[derive(Subcommand, Debug, PartialEq)]
pub enum Command {
    /// Output shell function definition for shell integration
    Init {
        /// Remaining arguments (e.g., path)
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Append init snippet to RC file
    Install {
        /// Remaining arguments (e.g., path)
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Clone a git repository into a dated directory
    Clone {
        /// Remaining arguments (url, name)
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Create a git worktree in a dated directory
    Worktree {
        /// Remaining arguments (repo, name)
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Execute command and output shell script
    Exec {
        /// Remaining arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Interactive directory selector
    Cd {
        /// Remaining arguments (query)
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Catch-all for unknown commands treated as search queries
    #[command(external_subcommand)]
    External(Vec<String>),
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

/// Parsed CLI arguments in normalized form
#[derive(Debug)]
#[allow(dead_code)]
pub struct CliArgs {
    /// The subcommand
    pub command: Option<NormalizedCommand>,
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

/// Normalized command enum (simpler than the clap one)
#[derive(Debug, PartialEq)]
pub enum NormalizedCommand {
    Init,
    Install,
    Clone,
    Worktree,
    Exec,
    Cd,
}

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

/// Parse command-line arguments.
///
/// This uses clap derive macros but preserves the custom behavior:
/// - --help/-h anywhere → help to stderr, exit 0
/// - --version/-v anywhere → version to stderr, exit 0
/// - no args → help to stderr, exit 2
/// - flags can appear before or after the command
pub fn parse_args() -> ParseResult {
    let raw_args: Vec<String> = env::args().skip(1).collect();

    // Check for --help/-h anywhere in args (before clap parsing)
    if raw_args.iter().any(|a| a == "--help" || a == "-h") {
        return ParseResult::Help(0);
    }

    // Check for --version/-v anywhere in args
    if raw_args.iter().any(|a| a == "--version" || a == "-v") {
        return ParseResult::Version;
    }

    // Try to parse with clap (suppressing errors for better UX)
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(_e) => {
            // If clap can't parse, check if it's a "no args" situation
            if raw_args.is_empty() {
                return ParseResult::Help(2);
            }
            // Otherwise, treat as an external subcommand with all args
            // This handles edge cases where clap might reject valid inputs
            return ParseResult::Run(CliArgs {
                command: None,
                path: None,
                no_colors: false,
                and_exit: false,
                and_keys: None,
                and_type: None,
                and_confirm: None,
                args: raw_args,
            });
        }
    };

    // Handle help/version flags (shouldn't reach here due to pre-check, but be safe)
    if cli.help {
        return ParseResult::Help(0);
    }
    if cli.version {
        return ParseResult::Version;
    }

    let no_colors = cli.no_colors || cli.no_expand_tokens;

    // Normalize the command and extract remaining args
    let (command, args) = match cli.command {
        Some(Command::Init { args }) => (Some(NormalizedCommand::Init), args),
        Some(Command::Install { args }) => (Some(NormalizedCommand::Install), args),
        Some(Command::Clone { args }) => (Some(NormalizedCommand::Clone), args),
        Some(Command::Worktree { args }) => (Some(NormalizedCommand::Worktree), args),
        Some(Command::Exec { args }) => (Some(NormalizedCommand::Exec), args),
        Some(Command::Cd { args }) => (Some(NormalizedCommand::Cd), args),
        Some(Command::External(args)) => (None, args),
        None => (None, vec![]),
    };

    // No args at all → help with exit code 2
    if command.is_none()
        && args.is_empty()
        && cli.path.is_none()
        && !cli.and_exit
        && cli.and_keys.is_none()
        && cli.and_type.is_none()
    {
        return ParseResult::Help(2);
    }

    ParseResult::Run(CliArgs {
        command,
        path: cli.path,
        no_colors,
        and_exit: cli.and_exit,
        and_keys: cli.and_keys,
        and_type: cli.and_type,
        and_confirm: cli.and_confirm,
        args,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to parse a given set of args (simulating command-line input).
    /// We use the same pre-processing logic as parse_args but without env::args.
    fn parse(args: &[&str]) -> ParseResult {
        let raw_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        // Check for --help/-h
        if raw_args.iter().any(|a| a == "--help" || a == "-h") {
            return ParseResult::Help(0);
        }

        // Check for --version/-v
        if raw_args.iter().any(|a| a == "--version" || a == "-v") {
            return ParseResult::Version;
        }

        // Build a full arg list with the binary name prepended for clap
        let full_args: Vec<String> = std::iter::once("lab".to_string())
            .chain(raw_args.iter().cloned())
            .collect();

        let cli = match Cli::try_parse_from(&full_args) {
            Ok(cli) => cli,
            Err(_) => {
                if raw_args.is_empty() {
                    return ParseResult::Help(2);
                }
                return ParseResult::Run(CliArgs {
                    command: None,
                    path: None,
                    no_colors: false,
                    and_exit: false,
                    and_keys: None,
                    and_type: None,
                    and_confirm: None,
                    args: raw_args,
                });
            }
        };

        if cli.help {
            return ParseResult::Help(0);
        }
        if cli.version {
            return ParseResult::Version;
        }

        let no_colors = cli.no_colors || cli.no_expand_tokens;

        let (command, args) = match cli.command {
            Some(Command::Init { args }) => (Some(NormalizedCommand::Init), args),
            Some(Command::Install { args }) => (Some(NormalizedCommand::Install), args),
            Some(Command::Clone { args }) => (Some(NormalizedCommand::Clone), args),
            Some(Command::Worktree { args }) => (Some(NormalizedCommand::Worktree), args),
            Some(Command::Exec { args }) => (Some(NormalizedCommand::Exec), args),
            Some(Command::Cd { args }) => (Some(NormalizedCommand::Cd), args),
            Some(Command::External(ext_args)) => (None, ext_args),
            None => (None, vec![]),
        };

        if command.is_none()
            && args.is_empty()
            && cli.path.is_none()
            && !cli.and_exit
            && cli.and_keys.is_none()
            && cli.and_type.is_none()
        {
            return ParseResult::Help(2);
        }

        ParseResult::Run(CliArgs {
            command,
            path: cli.path,
            no_colors,
            and_exit: cli.and_exit,
            and_keys: cli.and_keys,
            and_type: cli.and_type,
            and_confirm: cli.and_confirm,
            args,
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
                assert_eq!(args.command, Some(NormalizedCommand::Init));
                assert_eq!(args.args, vec!["/tmp/labs"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_clone_command() {
        match parse(&["clone", "https://github.com/user/repo"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(NormalizedCommand::Clone));
                assert_eq!(args.args, vec!["https://github.com/user/repo"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_exec_command() {
        match parse(&["exec", "cd", "query"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
                assert_eq!(args.args, vec!["cd", "query"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_cd_command() {
        match parse(&["cd", "myquery"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(NormalizedCommand::Cd));
                assert_eq!(args.args, vec!["myquery"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_unknown_command_treated_as_query() {
        match parse(&["myquery"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, None);
                assert!(args.args.contains(&"myquery".to_string()));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_path_equals_form() {
        match parse(&["--path=/custom/dir", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.path, Some("/custom/dir".to_string()));
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_path_space_form() {
        match parse(&["--path", "/custom/dir", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.path, Some("/custom/dir".to_string()));
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_no_colors_flag() {
        match parse(&["--no-colors", "exec"]) {
            ParseResult::Run(args) => {
                assert!(args.no_colors);
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_exit_flag() {
        match parse(&["--and-exit", "exec"]) {
            ParseResult::Run(args) => {
                assert!(args.and_exit);
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_keys_option() {
        match parse(&["--and-keys", "DOWN,ENTER", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.and_keys, Some("DOWN,ENTER".to_string()));
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_type_option() {
        match parse(&["--and-type", "hello", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.and_type, Some("hello".to_string()));
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_and_confirm_option() {
        match parse(&["--and-confirm", "YES", "exec"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.and_confirm, Some("YES".to_string()));
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
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
                assert_eq!(args.command, Some(NormalizedCommand::Exec));
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
        assert!(help.contains("lab"), "Help text must reference 'lab'");
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
        assert!(
            ver_parts.len() >= 2,
            "Version should have at least major.minor"
        );
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
                assert_eq!(args.command, Some(NormalizedCommand::Worktree));
                assert_eq!(args.args, vec!["feature-name"]);
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }

    #[test]
    fn test_install_command() {
        match parse(&["install"]) {
            ParseResult::Run(args) => {
                assert_eq!(args.command, Some(NormalizedCommand::Install));
                assert!(args.args.is_empty());
            }
            other => panic!("Expected Run, got {:?}", other),
        }
    }
}
