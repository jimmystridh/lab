//! Shell detection and init snippet generation.
//!
//! Detects the user's shell (bash, zsh, fish, powershell) from the SHELL
//! environment variable. Generates the appropriate shell wrapper function
//! that evals lab's stdout output on success.
