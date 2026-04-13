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

fn main() {
    // TODO: Parse CLI args and dispatch to commands
    println!("lab: ephemeral workspace manager");
}
