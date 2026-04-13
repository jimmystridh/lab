//! Git URI parsing and worktree detection.
//!
//! Parses HTTPS and SSH git URIs to extract user/repo components.
//! Detects whether a directory is a git repository or worktree.
//! Provides `is_git_uri()` for URL shorthand detection.
