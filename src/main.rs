//! lab - An ephemeral workspace manager.
//!
//! Entry point for the lab CLI/TUI tool. Handles CLI dispatch
//! by parsing arguments and routing to the appropriate command handler.

mod cli;
mod commands;
mod entries;
mod fuzzy;
mod git;
mod script;
mod shell;
mod tui;
mod util;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag for color support. When true, ANSI colors are disabled.
pub static NO_COLORS: AtomicBool = AtomicBool::new(false);

/// Resolve the labs path from --path flag, LAB_PATH env, or default ~/src/labs.
/// Auto-creates the directory if it doesn't exist.
fn resolve_labs_path(path_override: Option<&str>) -> PathBuf {
    let path = if let Some(p) = path_override {
        PathBuf::from(shellexpand_tilde(p))
    } else if let Ok(env_path) = env::var("LAB_PATH") {
        if env_path.is_empty() {
            default_labs_path()
        } else {
            PathBuf::from(shellexpand_tilde(&env_path))
        }
    } else {
        default_labs_path()
    };

    // Auto-create if it doesn't exist
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }

    path
}

/// Default labs path: ~/src/labs
fn default_labs_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join("src").join("labs")
    } else {
        PathBuf::from("~/src/labs")
    }
}

/// Resolve LAB_PROJECTS from env or default to parent of labs_path.
fn resolve_projects_path(labs_path: &Path) -> PathBuf {
    if let Ok(env_val) = env::var("LAB_PROJECTS") {
        if !env_val.is_empty() {
            return PathBuf::from(shellexpand_tilde(&env_val));
        }
    }
    // Default: parent of labs_path
    labs_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| labs_path.to_path_buf())
}

/// Simple tilde expansion for paths
fn shellexpand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

fn main() {
    // Check NO_COLOR env var early (before parsing args)
    if let Ok(val) = env::var("NO_COLOR") {
        if !val.is_empty() {
            NO_COLORS.store(true, Ordering::Relaxed);
        }
    }

    let result = cli::parse_args();

    match result {
        cli::ParseResult::Help(exit_code) => {
            eprint!("{}", cli::help_text());
            process::exit(exit_code);
        }
        cli::ParseResult::Version => {
            eprintln!("{}", cli::version_text());
            process::exit(0);
        }
        cli::ParseResult::Run(args) => {
            // Set no-colors from flag
            if args.no_colors {
                NO_COLORS.store(true, Ordering::Relaxed);
            }

            // Resolve labs path
            let labs_path = resolve_labs_path(args.path.as_deref());
            let _projects_path = resolve_projects_path(&labs_path);

            // Dispatch based on command
            match args.command {
                Some(cli::Command::Init) => {
                    // TODO: Implement in commands/init.rs
                    eprintln!("lab: init command not yet implemented");
                    process::exit(1);
                }
                Some(cli::Command::Install) => {
                    // TODO: Implement in commands/install.rs
                    eprintln!("lab: install command not yet implemented");
                    process::exit(1);
                }
                Some(cli::Command::Clone) => {
                    // TODO: Implement in commands/clone.rs
                    eprintln!("lab: clone command not yet implemented");
                    process::exit(1);
                }
                Some(cli::Command::Worktree) => {
                    // TODO: Implement in commands/worktree.rs
                    eprintln!("lab: worktree command not yet implemented");
                    process::exit(1);
                }
                Some(cli::Command::Exec) => {
                    // TODO: Implement exec routing in commands/cd.rs
                    // For now, handle test flags minimally
                    if args.and_exit {
                        // Render one frame and exit (TUI test mode)
                        eprintln!("lab: TUI not yet implemented");
                        process::exit(1);
                    }
                    eprintln!("lab: exec command not yet implemented");
                    process::exit(1);
                }
                None => {
                    // Default: treat remaining args as search query → TUI selector
                    // Same as `lab exec [query]`
                    if args.and_exit {
                        eprintln!("lab: TUI not yet implemented");
                        process::exit(1);
                    }
                    eprintln!("lab: TUI selector not yet implemented");
                    process::exit(1);
                }
            }
        }
    }
}
