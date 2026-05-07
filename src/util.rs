//! Utility functions shared across modules.
//!
//! Provides path quoting (single-quote with proper escaping),
//! date formatting (YYYY-MM-DD prefix generation), relative timestamp
//! formatting, and name collision resolution with numeric suffixes.

use std::path::Path;

/// Quote a path for safe use in shell scripts.
///
/// Wraps the string in single quotes and escapes any internal single quotes
/// using the `'"'"'` idiom (end single-quote, double-quote a single-quote,
/// resume single-quote).
///
/// # Examples
/// ```
/// assert_eq!(quote_path("/simple/path"), "'/simple/path'");
/// assert_eq!(quote_path("it's"), "'it'\"'\"'s'");
/// ```
pub fn quote_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len() + 2);
    result.push('\'');
    for ch in path.chars() {
        if ch == '\'' {
            result.push_str("'\"'\"'");
        } else {
            result.push(ch);
        }
    }
    result.push('\'');
    result
}

/// Resolve a unique directory name under `labs_path` by handling collisions.
///
/// When creating a new directory with `YYYY-MM-DD-base`, if that name already
/// exists under `labs_path`:
/// - If `base` ends with trailing digits (e.g., `feature1`): increment the
///   trailing number until a unique name is found (`feature2`, `feature3`, ...).
/// - If `base` has no trailing digits (e.g., `nonum`): append `-2`, `-3`, etc.
///
/// Returns the (possibly modified) base name (without date prefix).
pub fn resolve_unique_name(labs_path: &str, date_prefix: &str, base: &str) -> String {
    let initial = format!("{}-{}", date_prefix, base);
    if !Path::new(labs_path).join(&initial).exists() {
        return base.to_string();
    }

    // Check if base ends with trailing digits
    let trailing_start = base
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, _)| i);

    if let Some(start) = trailing_start {
        // Base ends with digits — increment them
        let stem = &base[..start];
        let num: u64 = base[start..].parse().unwrap_or(0);
        let mut candidate_num = num + 1;
        loop {
            let candidate_base = format!("{}{}", stem, candidate_num);
            let candidate_full =
                Path::new(labs_path).join(format!("{}-{}", date_prefix, candidate_base));
            if !candidate_full.exists() {
                return candidate_base;
            }
            candidate_num += 1;
        }
    } else {
        // No trailing digits — use -2, -3 style suffix on the full dated name
        let mut i = 2u64;
        loop {
            let candidate = format!("{}-{}-{}", date_prefix, base, i);
            if !Path::new(labs_path).join(&candidate).exists() {
                // Strip the date prefix back off to return just the base portion
                return format!("{}-{}", base, i);
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_path_simple() {
        assert_eq!(quote_path("/tmp/foo"), "'/tmp/foo'");
    }

    #[test]
    fn test_quote_path_with_spaces() {
        assert_eq!(quote_path("/tmp/my dir"), "'/tmp/my dir'");
    }

    #[test]
    fn test_quote_path_with_single_quote() {
        assert_eq!(quote_path("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn test_quote_path_with_multiple_single_quotes() {
        assert_eq!(quote_path("a'b'c"), "'a'\"'\"'b'\"'\"'c'");
    }

    #[test]
    fn test_quote_path_empty() {
        assert_eq!(quote_path(""), "''");
    }

    #[test]
    fn test_quote_path_only_single_quote() {
        assert_eq!(quote_path("'"), "''\"'\"''");
    }

    #[test]
    fn test_quote_path_with_special_chars() {
        assert_eq!(quote_path("/tmp/$HOME"), "'/tmp/$HOME'");
    }

    #[test]
    fn test_quote_path_with_double_quotes() {
        assert_eq!(quote_path("/tmp/\"foo\""), "'/tmp/\"foo\"'");
    }

    #[test]
    fn test_quote_path_with_backslash() {
        assert_eq!(quote_path("/tmp/foo\\bar"), "'/tmp/foo\\bar'");
    }

    #[test]
    fn test_quote_path_unicode() {
        assert_eq!(quote_path("/tmp/café"), "'/tmp/café'");
    }

    // ---- resolve_unique_name tests ----

    #[test]
    fn test_resolve_unique_name_no_collision() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_unique_name(dir.path().to_str().unwrap(), "2025-01-15", "feature");
        assert_eq!(result, "feature");
    }

    #[test]
    fn test_resolve_unique_name_trailing_digits_bumps_number() {
        let dir = tempfile::tempdir().unwrap();
        // Create the colliding directory
        std::fs::create_dir(dir.path().join("2025-01-15-feature1")).unwrap();
        let result = resolve_unique_name(dir.path().to_str().unwrap(), "2025-01-15", "feature1");
        assert_eq!(result, "feature2");
    }

    #[test]
    fn test_resolve_unique_name_trailing_digits_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("2025-01-15-feature1")).unwrap();
        std::fs::create_dir(dir.path().join("2025-01-15-feature2")).unwrap();
        let result = resolve_unique_name(dir.path().to_str().unwrap(), "2025-01-15", "feature1");
        assert_eq!(result, "feature3");
    }

    #[test]
    fn test_resolve_unique_name_no_digits_appends_dash_2() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("2025-01-15-nonum")).unwrap();
        let result = resolve_unique_name(dir.path().to_str().unwrap(), "2025-01-15", "nonum");
        assert_eq!(result, "nonum-2");
    }

    #[test]
    fn test_resolve_unique_name_no_digits_increments_suffix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("2025-01-15-nonum")).unwrap();
        std::fs::create_dir(dir.path().join("2025-01-15-nonum-2")).unwrap();
        let result = resolve_unique_name(dir.path().to_str().unwrap(), "2025-01-15", "nonum");
        assert_eq!(result, "nonum-3");
    }

    #[test]
    fn test_resolve_unique_name_large_trailing_number() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("2025-01-15-test99")).unwrap();
        let result = resolve_unique_name(dir.path().to_str().unwrap(), "2025-01-15", "test99");
        assert_eq!(result, "test100");
    }

    #[test]
    fn test_resolve_unique_name_stem_with_hyphens_and_digits() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("2025-01-15-my-feature1")).unwrap();
        let result = resolve_unique_name(dir.path().to_str().unwrap(), "2025-01-15", "my-feature1");
        assert_eq!(result, "my-feature2");
    }
}
