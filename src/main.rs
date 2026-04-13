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

/// Dispatch exec command routing.
///
/// Handles sub-dispatch for clone, worktree, cd, dot shorthand, URL shorthand,
/// and the default cd/TUI path.
fn dispatch_exec(
    args: &[String],
    labs_path: &str,
    and_exit: bool,
    and_keys: Option<&str>,
    and_type: Option<&str>,
    and_confirm: Option<&str>,
) -> i32 {
    let first = args.first().map(|s| s.as_str());

    // Sub-dispatch: exec clone → cmd_clone
    if first == Some("clone") {
        let sub_args: Vec<&str> = args.iter().skip(1).map(|s| s.as_str()).collect();
        let uri = sub_args.first().copied();
        let custom_name = sub_args.get(1).copied();
        return commands::clone::cmd_clone(uri, custom_name, labs_path);
    }

    // Sub-dispatch: exec worktree → cmd_worktree
    if first == Some("worktree") {
        let sub_args: Vec<String> = args.iter().skip(1).cloned().collect();
        return commands::worktree::cmd_worktree(&sub_args, labs_path);
    }

    // Sub-dispatch: exec cd → cd path (with remaining args)
    if first == Some("cd") {
        let sub_args: Vec<String> = args.iter().skip(1).cloned().collect();

        // URL shorthand inside cd: exec cd <url> → clone
        if let Some(url_arg) = sub_args.first() {
            if git::is_git_uri(url_arg) {
                let uri = Some(url_arg.as_str());
                let custom_name = sub_args.get(1).map(|s| s.as_str());
                return commands::clone::cmd_clone(uri, custom_name, labs_path);
            }
        }

        return commands::cd::cmd_cd(&sub_args, labs_path, and_exit, and_keys, and_type, and_confirm);
    }

    // Dot shorthand: exec . [name] or exec ./subdir name
    if let Some(f) = first {
        if f.starts_with('.') {
            let dot_arg = f.to_string();
            let rest: Vec<String> = args.iter().skip(1).cloned().collect();
            return commands::worktree::cmd_dot(&dot_arg, &rest, labs_path);
        }
    }

    // URL shorthand: if first arg looks like git URI → clone
    if let Some(f) = first {
        if git::is_git_uri(f) {
            let uri = Some(f);
            let custom_name = args.get(1).map(|s| s.as_str());
            return commands::clone::cmd_clone(uri, custom_name, labs_path);
        }
    }

    // Default: cd/TUI path with remaining args as query
    commands::cd::cmd_cd(args, labs_path, and_exit, and_keys, and_type, and_confirm)
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
                Some(cli::NormalizedCommand::Init) => {
                    commands::init::cmd_init(&args.args, &labs_path.to_string_lossy());
                    process::exit(0);
                }
                Some(cli::NormalizedCommand::Install) => {
                    let exit_code =
                        commands::install::cmd_install(&args.args, &labs_path.to_string_lossy());
                    process::exit(exit_code);
                }
                Some(cli::NormalizedCommand::Clone) => {
                    let uri = args.args.first().map(|s| s.as_str());
                    let custom_name = args.args.get(1).map(|s| s.as_str());
                    let exit_code = commands::clone::cmd_clone(
                        uri,
                        custom_name,
                        &labs_path.to_string_lossy(),
                    );
                    process::exit(exit_code);
                }
                Some(cli::NormalizedCommand::Worktree) => {
                    let exit_code =
                        commands::worktree::cmd_worktree(&args.args, &labs_path.to_string_lossy());
                    process::exit(exit_code);
                }
                Some(cli::NormalizedCommand::Exec) => {
                    let labs = labs_path.to_string_lossy().to_string();
                    let exit_code = dispatch_exec(
                        &args.args,
                        &labs,
                        args.and_exit,
                        args.and_keys.as_deref(),
                        args.and_type.as_deref(),
                        args.and_confirm.as_deref(),
                    );
                    process::exit(exit_code);
                }
                Some(cli::NormalizedCommand::Cd) => {
                    // cd command: same as exec cd
                    let labs = labs_path.to_string_lossy().to_string();
                    let exit_code = commands::cd::cmd_cd(
                        &args.args,
                        &labs,
                        args.and_exit,
                        args.and_keys.as_deref(),
                        args.and_type.as_deref(),
                        args.and_confirm.as_deref(),
                    );
                    process::exit(exit_code);
                }
                None => {
                    // Default: treat remaining args as search query
                    // Same as `lab exec [query]`
                    let labs = labs_path.to_string_lossy().to_string();
                    let exit_code = dispatch_exec(
                        &args.args,
                        &labs,
                        args.and_exit,
                        args.and_keys.as_deref(),
                        args.and_type.as_deref(),
                        args.and_confirm.as_deref(),
                    );
                    process::exit(exit_code);
                }
            }
        }
    }
}
