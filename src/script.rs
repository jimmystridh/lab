//! Shell script emission for cd, mkdir, clone, worktree, delete, rename, and graduate.
//!
//! Generates shell scripts written to stdout that the shell wrapper function
//! evaluates. Scripts follow a consistent format: warning header comment,
//! commands chained with `&& \`, and 2-space indented continuations.
