//! Fuzzy matching engine for filtering and scoring directory entries.
//!
//! Implements case-insensitive subsequence matching with scoring bonuses
//! for word boundaries, proximity of matched characters, match density,
//! and name length. Returns match scores and highlight positions for
//! rendering matched characters in the TUI.

use crate::entries::Entry;

/// Pre-computed sqrt table for proximity bonus: 2.0 / sqrt(gap + 1) for gap 0..=63.
const SQRT_TABLE_SIZE: usize = 64;

/// Lazily initialize the sqrt table (computed once at first use).
fn sqrt_table() -> &'static [f64; SQRT_TABLE_SIZE] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[f64; SQRT_TABLE_SIZE]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0.0; SQRT_TABLE_SIZE];
        for (i, val) in table.iter_mut().enumerate() {
            *val = 2.0 / ((i as f64) + 1.0).sqrt();
        }
        table
    })
}

/// Get the proximity bonus for a given gap.
/// Uses the precomputed table for gaps 0-63, falls back to direct computation.
#[inline]
fn proximity_bonus(gap: usize) -> f64 {
    if gap < SQRT_TABLE_SIZE {
        sqrt_table()[gap]
    } else {
        2.0 / ((gap as f64) + 1.0).sqrt()
    }
}

/// Fuzzy matching engine that holds a set of entries and matches them against queries.
pub struct Fuzzy {
    /// The entries to match against, with their lowercase names pre-computed.
    entries: Vec<FuzzyEntry>,
}

/// Internal representation of an entry for fuzzy matching.
struct FuzzyEntry {
    /// The original entry name.
    text: String,
    /// Lowercased version for case-insensitive matching.
    text_lower: String,
    /// Base score from recency + date prefix.
    base_score: f64,
    /// Index into the original entries vec.
    index: usize,
}

/// Result of a fuzzy match: score and highlight positions.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MatchResult {
    /// Index of the entry in the original entries vec.
    pub index: usize,
    /// Total score (fuzzy + base).
    pub score: f64,
    /// Positions in the entry name where query characters matched.
    pub positions: Vec<usize>,
}

impl Fuzzy {
    /// Create a new Fuzzy instance from a slice of entries.
    pub fn new(entries: &[Entry]) -> Self {
        let fuzzy_entries = entries
            .iter()
            .enumerate()
            .map(|(i, e)| FuzzyEntry {
                text: e.name.clone(),
                text_lower: e.name.to_lowercase(),
                base_score: e.base_score,
                index: i,
            })
            .collect();

        Self {
            entries: fuzzy_entries,
        }
    }

    /// Match all entries against the given query, returning scored results
    /// sorted by score descending, limited to at most `limit` results.
    pub fn match_entries(&self, query: &str, limit: usize) -> Vec<MatchResult> {
        let query_lower: Vec<char> = query.to_lowercase().chars().collect();

        let mut results: Vec<MatchResult> = self
            .entries
            .iter()
            .filter_map(|entry| self.calculate_match(entry, &query_lower))
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit results
        results.truncate(limit);

        results
    }

    /// Calculate the match score and highlight positions for a single entry.
    ///
    /// Returns `None` if the query doesn't match (not all chars found in sequence).
    /// Returns `Some(MatchResult)` with score and positions on match.
    ///
    /// For an empty query, returns the entry with its base_score and no highlights.
    fn calculate_match(&self, entry: &FuzzyEntry, query_chars: &[char]) -> Option<MatchResult> {
        let mut positions = Vec::new();
        let mut score = entry.base_score;

        // Empty query: match all entries with base score only
        if query_chars.is_empty() {
            return Some(MatchResult {
                index: entry.index,
                score,
                positions,
            });
        }

        let text_chars: Vec<char> = entry.text_lower.chars().collect();
        let query_len = query_chars.len();

        let mut last_pos: Option<usize> = None;
        let mut pos = 0;

        for &qc in query_chars {
            // Find next occurrence of query char starting from pos
            let found = text_chars[pos..].iter().position(|&c| c == qc);
            let found = match found {
                Some(offset) => pos + offset,
                None => return None, // No match
            };

            positions.push(found);

            // Base match point: +1.0
            score += 1.0;

            // Word boundary bonus: +1.0 if at pos 0 or after non-[a-z0-9]
            if found == 0 || !is_word_char(text_chars[found - 1]) {
                score += 1.0;
            }

            // Proximity bonus: 2.0/sqrt(gap+1) for consecutive matches
            if let Some(lp) = last_pos {
                let gap = found - lp - 1;
                score += proximity_bonus(gap);
            }

            last_pos = Some(found);
            pos = found + 1;
        }

        // Apply density multiplier: query_len / (last_pos + 1)
        if let Some(lp) = last_pos {
            let fuzzy_component = score - entry.base_score;
            let density = query_len as f64 / (lp as f64 + 1.0);
            let length_factor = 10.0 / (entry.text.len() as f64 + 10.0);
            // Apply multipliers to fuzzy score only, then add back base score
            score = fuzzy_component * density * length_factor + entry.base_score;
        }

        Some(MatchResult {
            index: entry.index,
            score,
            positions,
        })
    }
}

/// Check if a character is a word character [a-z0-9].
/// Used for word boundary detection.
#[inline]
fn is_word_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entries::Entry;
    use std::path::PathBuf;
    use std::time::SystemTime;

    /// Helper to create test entries with specific names and base scores.
    fn make_entry(name: &str, base_score: f64) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{}", name)),
            is_symlink: false,
            mtime: SystemTime::now(),
            base_score,
        }
    }

    #[test]
    fn test_case_insensitive_matching() {
        let entries = vec![make_entry("beta", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("BETA", 10);
        assert_eq!(results.len(), 1, "BETA should match beta");
        assert_eq!(results[0].positions, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_mixed_case_matching() {
        let entries = vec![make_entry("alpha", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("AlPhA", 10);
        assert_eq!(results.len(), 1, "AlPhA should match alpha");
    }

    #[test]
    fn test_partial_subsequence_matching() {
        let entries = vec![make_entry("gamma", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("gam", 10);
        assert_eq!(results.len(), 1, "gam should match gamma");
        assert_eq!(results[0].positions, vec![0, 1, 2]);
    }

    #[test]
    fn test_non_matching_query_returns_none() {
        let entries = vec![make_entry("alpha", 0.0), make_entry("beta", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("xyz", 10);
        assert!(results.is_empty(), "xyz should not match any entry");
    }

    #[test]
    fn test_strict_subsequence_requirement() {
        // "ba" should not match "ab-only" because 'b' comes before 'a'
        let entries = vec![make_entry("ab-only", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("ba", 10);
        // 'b' is at position 1, 'a' is at position 0 — but subsequence requires order
        // Actually 'b' is at 1, then we look for 'a' from position 2 onward — not found
        assert!(results.is_empty(), "'ba' should not match 'ab-only'");
    }

    #[test]
    fn test_word_boundary_bonus_at_pos_0() {
        // Match at position 0 should get word boundary bonus
        let entries = vec![make_entry("project", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("p", 10);
        assert_eq!(results.len(), 1);
        // Score: base_score(0) + 1.0(base char) + 1.0(word boundary at pos 0) = 2.0
        // density: 1/(0+1) = 1.0, length: 10/(7+10) ≈ 0.588
        // fuzzy_component = 2.0 * 1.0 * 0.588 ≈ 1.176
        let expected = 2.0 * 1.0 * (10.0 / 17.0);
        assert!(
            (results[0].score - expected).abs() < 0.01,
            "score should be ~{:.3}, got {:.3}",
            expected,
            results[0].score
        );
    }

    #[test]
    fn test_word_boundary_bonus_after_non_alnum() {
        // "b" against "foo-bar" should match at position 4 (after '-')
        let entries = vec![make_entry("foo-bar", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("b", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].positions, vec![4]);
        // Score: 1.0(base) + 1.0(word boundary after '-') = 2.0
        // density: 1/(4+1) = 0.2, length: 10/(7+10) ≈ 0.588
        // fuzzy_component = 2.0 * 0.2 * 0.588 ≈ 0.235
        let expected = 2.0 * (1.0 / 5.0) * (10.0 / 17.0);
        assert!(
            (results[0].score - expected).abs() < 0.01,
            "score should be ~{:.3}, got {:.3}",
            expected,
            results[0].score
        );
    }

    #[test]
    fn test_proximity_bonus_consecutive() {
        // "proj" against "project" — all consecutive matches (positions 0,1,2,3)
        let entries = vec![make_entry("project", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("proj", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].positions, vec![0, 1, 2, 3]);
        // Consecutive matches should have higher proximity bonus
    }

    #[test]
    fn test_proximity_bonus_scattered() {
        // "pjt" against "project" — scattered matches
        let entries = vec![make_entry("project", 0.0)];
        let fuzzy = Fuzzy::new(&entries);

        let results_consec = fuzzy.match_entries("proj", 10);
        let results_scatter = fuzzy.match_entries("pjt", 10);

        assert!(
            results_consec[0].score > results_scatter[0].score,
            "consecutive matches ({:.3}) should score higher than scattered ({:.3})",
            results_consec[0].score,
            results_scatter[0].score
        );
    }

    #[test]
    fn test_length_penalty_prefers_shorter_names() {
        let entries = vec![
            make_entry("project", 0.0),
            make_entry("project-with-long-suffix", 0.0),
        ];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("proj", 10);
        assert_eq!(results.len(), 2);
        // Shorter name should be first (higher score)
        assert_eq!(results[0].positions, vec![0, 1, 2, 3]);
        assert!(
            results[0].score > results[1].score,
            "shorter name ({:.3}) should score higher than longer ({:.3})",
            results[0].score,
            results[1].score
        );
    }

    #[test]
    fn test_density_multiplier_prefers_concentrated_matches() {
        let entries = vec![make_entry("abcdef", 0.0), make_entry("a----b", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("ab", 10);
        assert_eq!(results.len(), 2);
        // "abcdef" has positions [0,1] (density=2/2=1.0)
        // "a----b" has positions [0,5] (density=2/6≈0.33)
        assert!(
            results[0].score > results[1].score,
            "concentrated matches ({:.3}) should score higher than scattered ({:.3})",
            results[0].score,
            results[1].score
        );
    }

    #[test]
    fn test_date_prefix_bonus() {
        let entries = vec![
            make_entry("2025-01-15-project", 2.0), // base_score includes +2.0 date bonus
            make_entry("project", 0.0),            // base_score with no date bonus
        ];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("proj", 10);
        // Date-prefixed entry should rank higher due to base_score bonus
        assert_eq!(results.len(), 2);
        assert!(
            results[0].score > results[1].score,
            "date-prefixed entry should score higher"
        );
    }

    #[test]
    fn test_recency_bonus_from_base_score() {
        // Two entries with different base scores (simulating different mtimes)
        let entries = vec![
            make_entry("alpha", 3.0), // recent
            make_entry("beta", 0.5),  // older
        ];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("", 10);
        // With empty query, only base_score matters
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].index, 0,
            "higher base_score entry should come first"
        );
    }

    #[test]
    fn test_empty_query_returns_all_entries() {
        let entries = vec![
            make_entry("alpha", 1.0),
            make_entry("beta", 2.0),
            make_entry("gamma", 0.5),
        ];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("", 10);

        assert_eq!(results.len(), 3, "empty query should return all entries");
        // Should be sorted by base_score descending
        assert_eq!(results[0].index, 1, "beta (2.0) should be first");
        assert_eq!(results[1].index, 0, "alpha (1.0) should be second");
        assert_eq!(results[2].index, 2, "gamma (0.5) should be third");
    }

    #[test]
    fn test_empty_query_returns_empty_positions() {
        let entries = vec![make_entry("alpha", 1.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("", 10);
        assert!(
            results[0].positions.is_empty(),
            "empty query should return empty positions"
        );
    }

    #[test]
    fn test_result_limiting() {
        let entries = vec![
            make_entry("alpha", 1.0),
            make_entry("beta", 2.0),
            make_entry("gamma", 0.5),
            make_entry("delta", 3.0),
            make_entry("epsilon", 1.5),
        ];
        let fuzzy = Fuzzy::new(&entries);

        // Limit to 3
        let results = fuzzy.match_entries("", 3);
        assert_eq!(results.len(), 3, "should return at most 3 results");

        // Top 3 by base_score: delta(3.0), beta(2.0), epsilon(1.5)
        assert_eq!(results[0].index, 3, "delta should be first");
        assert_eq!(results[1].index, 1, "beta should be second");
        assert_eq!(results[2].index, 4, "epsilon should be third");
    }

    #[test]
    fn test_sqrt_table_precomputed() {
        let table = sqrt_table();
        // Verify a few known values
        assert!((table[0] - 2.0).abs() < 0.0001, "gap 0: 2.0/sqrt(1) = 2.0");
        assert!(
            (table[1] - 2.0 / 2.0_f64.sqrt()).abs() < 0.0001,
            "gap 1: 2.0/sqrt(2)"
        );
        assert!(
            (table[3] - 2.0 / 4.0_f64.sqrt()).abs() < 0.0001,
            "gap 3: 2.0/sqrt(4) = 1.0"
        );
        assert!(
            (table[63] - 2.0 / 64.0_f64.sqrt()).abs() < 0.0001,
            "gap 63: 2.0/sqrt(64) = 0.25"
        );
    }

    #[test]
    fn test_proximity_bonus_large_gap() {
        // Gap > 63 should use direct computation
        let bonus = proximity_bonus(100);
        let expected = 2.0 / 101.0_f64.sqrt();
        assert!(
            (bonus - expected).abs() < 0.0001,
            "large gap should use direct computation"
        );
    }

    #[test]
    fn test_single_char_query() {
        let entries = vec![make_entry("alpha", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("a", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].positions, vec![0]);
    }

    #[test]
    fn test_full_name_match() {
        let entries = vec![make_entry("beta", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("beta", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].positions, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_no_entries() {
        let entries: Vec<Entry> = vec![];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("test", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_query_with_no_entries() {
        let entries: Vec<Entry> = vec![];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_longer_than_name() {
        let entries = vec![make_entry("ab", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("abc", 10);
        assert!(
            results.is_empty(),
            "query longer than name should not match"
        );
    }

    #[test]
    fn test_is_word_char() {
        assert!(is_word_char('a'));
        assert!(is_word_char('z'));
        assert!(is_word_char('0'));
        assert!(is_word_char('9'));
        assert!(!is_word_char('-'));
        assert!(!is_word_char('_'));
        assert!(!is_word_char('.'));
        assert!(!is_word_char(' '));
        assert!(!is_word_char('A')); // Only lowercase
    }

    #[test]
    fn test_date_prefixed_entry_matching() {
        let entries = vec![make_entry("2025-11-15-beta", 2.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("BETA", 10);
        assert_eq!(results.len(), 1, "BETA should match 2025-11-15-beta");
    }

    #[test]
    fn test_multiple_entries_ranking() {
        // Test that scores are properly ranked
        let entries = vec![
            make_entry("project", 0.0),
            make_entry("my-project", 0.0),
            make_entry("a-pretty-long-project-name", 0.0),
        ];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("proj", 10);

        assert_eq!(results.len(), 3);
        // "project" should score highest (shorter, denser match)
        assert_eq!(
            results[0].index, 0,
            "shortest match 'project' should rank first"
        );
    }

    #[test]
    fn test_limit_zero_returns_empty() {
        let entries = vec![make_entry("alpha", 1.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("a", 0);
        assert!(results.is_empty(), "limit 0 should return empty");
    }

    #[test]
    fn test_word_boundary_no_bonus_mid_word() {
        // 'r' in "project" at position 1 is after 'p' which is [a-z] — no boundary bonus
        let entries = vec![make_entry("project", 0.0)];
        let fuzzy = Fuzzy::new(&entries);
        let results = fuzzy.match_entries("r", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].positions, vec![1]);
        // Score: 1.0(base char) only, no word boundary
        // density: 1/(1+1)=0.5, length: 10/17≈0.588
        let expected = 1.0 * 0.5 * (10.0 / 17.0);
        assert!(
            (results[0].score - expected).abs() < 0.01,
            "mid-word char should not get boundary bonus, expected ~{:.3}, got {:.3}",
            expected,
            results[0].score
        );
    }
}
