//! Fuzzy matching engine for filtering and scoring directory entries.
//!
//! Implements case-insensitive subsequence matching with scoring bonuses
//! for word boundaries, proximity of matched characters, match density,
//! and name length. Returns match scores and highlight positions for
//! rendering matched characters in the TUI.
