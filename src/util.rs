//! Utility functions shared across modules.
//!
//! Provides path quoting (single-quote with proper escaping),
//! date formatting (YYYY-MM-DD prefix generation), relative timestamp
//! formatting, and name collision resolution with numeric suffixes.

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
}
