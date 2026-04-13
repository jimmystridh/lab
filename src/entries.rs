//! Directory entry loading and base score calculation.
//!
//! Handles reading the LAB_PATH directory, filtering hidden dirs and files,
//! detecting symlinks, computing mtime-based recency scores, and applying
//! date-prefix bonuses. Provides the `Entry` struct used throughout the app.
