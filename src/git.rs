//! Git URI parsing and worktree detection.
//!
//! Parses HTTPS and SSH git URIs to extract user/repo components.
//! Detects whether a directory is a git repository or worktree.
//! Provides `is_git_uri()` for URL shorthand detection.

/// Parsed git URI with host, user, and repo components.
#[derive(Debug, PartialEq)]
pub struct ParsedGitUri {
    pub host: String,
    pub user: String,
    pub repo: String,
}

/// Parse a git URI into its components (host, user, repo).
///
/// Supports the following formats:
/// - `https://github.com/user/repo`
/// - `https://github.com/user/repo.git`
/// - `git@github.com:user/repo`
/// - `git@github.com:user/repo.git`
/// - `https://gitlab.com/user/repo`
/// - `git@gitlab.com:user/repo`
/// - Generic `https://host/user/repo`
/// - Generic `git@host:user/repo`
///
/// The `.git` suffix is always stripped from the repo name.
///
/// Returns `None` if the URI cannot be parsed.
pub fn parse_git_uri(uri: &str) -> Option<ParsedGitUri> {
    // Strip .git suffix if present
    let uri = uri.strip_suffix(".git").unwrap_or(uri);

    // Try HTTPS format: https://host/user/repo
    if let Some(rest) = uri.strip_prefix("https://").or_else(|| uri.strip_prefix("http://")) {
        // Split host from path
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() != 2 {
            return None;
        }
        let host = parts[0];
        let path = parts[1];

        // For GitHub/GitLab: user/repo
        // For generic: user/repo
        let path_parts: Vec<&str> = path.splitn(3, '/').collect();
        if path_parts.len() < 2 || path_parts[0].is_empty() || path_parts[1].is_empty() {
            return None;
        }

        return Some(ParsedGitUri {
            host: host.to_string(),
            user: path_parts[0].to_string(),
            repo: path_parts[1].to_string(),
        });
    }

    // Try SSH format: git@host:user/repo
    if let Some(rest) = uri.strip_prefix("git@") {
        // Split host from path at ':'
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() != 2 {
            return None;
        }
        let host = parts[0];
        let path = parts[1];

        let path_parts: Vec<&str> = path.splitn(3, '/').collect();
        if path_parts.len() < 2 || path_parts[0].is_empty() || path_parts[1].is_empty() {
            return None;
        }

        return Some(ParsedGitUri {
            host: host.to_string(),
            user: path_parts[0].to_string(),
            repo: path_parts[1].to_string(),
        });
    }

    None
}

/// Check whether a string looks like a git URI.
///
/// Returns `true` if the string:
/// - Starts with `https://` or `http://` or `git@`, OR
/// - Contains `github.com` or `gitlab.com`, OR
/// - Ends with `.git`
pub fn is_git_uri(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.starts_with("https://")
        || s.starts_with("http://")
        || s.starts_with("git@")
        || s.contains("github.com")
        || s.contains("gitlab.com")
        || s.ends_with(".git")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_git_uri tests ----

    #[test]
    fn test_parse_https_github() {
        let parsed = parse_git_uri("https://github.com/user/repo").unwrap();
        assert_eq!(parsed.host, "github.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_https_github_with_git_suffix() {
        let parsed = parse_git_uri("https://github.com/user/repo.git").unwrap();
        assert_eq!(parsed.host, "github.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_ssh_github() {
        let parsed = parse_git_uri("git@github.com:user/repo").unwrap();
        assert_eq!(parsed.host, "github.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_ssh_github_with_git_suffix() {
        let parsed = parse_git_uri("git@github.com:user/repo.git").unwrap();
        assert_eq!(parsed.host, "github.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_https_gitlab() {
        let parsed = parse_git_uri("https://gitlab.com/user/repo").unwrap();
        assert_eq!(parsed.host, "gitlab.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_ssh_gitlab() {
        let parsed = parse_git_uri("git@gitlab.com:user/repo").unwrap();
        assert_eq!(parsed.host, "gitlab.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_generic_https_host() {
        let parsed = parse_git_uri("https://example.com/myuser/myrepo").unwrap();
        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.user, "myuser");
        assert_eq!(parsed.repo, "myrepo");
    }

    #[test]
    fn test_parse_generic_ssh_host() {
        let parsed = parse_git_uri("git@example.com:myuser/myrepo").unwrap();
        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.user, "myuser");
        assert_eq!(parsed.repo, "myrepo");
    }

    #[test]
    fn test_parse_generic_https_with_git_suffix() {
        let parsed = parse_git_uri("https://example.com/user/repo.git").unwrap();
        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_generic_ssh_with_git_suffix() {
        let parsed = parse_git_uri("git@example.com:user/repo.git").unwrap();
        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_invalid_bare_word() {
        assert!(parse_git_uri("not-a-valid-uri").is_none());
    }

    #[test]
    fn test_parse_invalid_no_path() {
        assert!(parse_git_uri("https://github.com").is_none());
    }

    #[test]
    fn test_parse_invalid_single_segment() {
        assert!(parse_git_uri("https://github.com/user").is_none());
    }

    #[test]
    fn test_parse_invalid_empty_user() {
        assert!(parse_git_uri("https://github.com//repo").is_none());
    }

    #[test]
    fn test_parse_invalid_empty_repo() {
        assert!(parse_git_uri("https://github.com/user/").is_none());
    }

    #[test]
    fn test_parse_ssh_no_colon() {
        assert!(parse_git_uri("git@github.com/user/repo").is_none());
    }

    #[test]
    fn test_parse_ssh_no_path() {
        assert!(parse_git_uri("git@github.com:").is_none());
    }

    #[test]
    fn test_parse_ssh_single_segment() {
        assert!(parse_git_uri("git@github.com:user").is_none());
    }

    #[test]
    fn test_parse_http_scheme() {
        let parsed = parse_git_uri("http://github.com/user/repo").unwrap();
        assert_eq!(parsed.host, "github.com");
        assert_eq!(parsed.user, "user");
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn test_parse_strips_git_suffix_only() {
        // .git should only be stripped from the end
        let parsed = parse_git_uri("https://github.com/user/my.git.project").unwrap();
        assert_eq!(parsed.repo, "my.git.project");
    }

    #[test]
    fn test_parse_extra_path_segments_ignored() {
        // Extra segments after user/repo should be ignored
        let parsed = parse_git_uri("https://github.com/user/repo/extra/path").unwrap();
        assert_eq!(parsed.user, "user");
        // The repo captures "repo" because we split into max 3 parts
        assert_eq!(parsed.repo, "repo");
    }

    // ---- is_git_uri tests ----

    #[test]
    fn test_is_git_uri_https_github() {
        assert!(is_git_uri("https://github.com/user/repo"));
    }

    #[test]
    fn test_is_git_uri_ssh_github() {
        assert!(is_git_uri("git@github.com:user/repo"));
    }

    #[test]
    fn test_is_git_uri_https_gitlab() {
        assert!(is_git_uri("https://gitlab.com/user/repo"));
    }

    #[test]
    fn test_is_git_uri_ssh_gitlab() {
        assert!(is_git_uri("git@gitlab.com:user/repo"));
    }

    #[test]
    fn test_is_git_uri_dot_git_suffix() {
        assert!(is_git_uri("https://example.com/user/repo.git"));
    }

    #[test]
    fn test_is_git_uri_generic_https() {
        assert!(is_git_uri("https://example.com/user/repo"));
    }

    #[test]
    fn test_is_git_uri_generic_ssh() {
        assert!(is_git_uri("git@example.com:user/repo"));
    }

    #[test]
    fn test_is_git_uri_plain_word() {
        assert!(!is_git_uri("not-a-uri"));
    }

    #[test]
    fn test_is_git_uri_empty() {
        assert!(!is_git_uri(""));
    }

    #[test]
    fn test_is_git_uri_contains_github_in_text() {
        // "github.com" anywhere triggers detection
        assert!(is_git_uri("check github.com/user/repo"));
    }

    #[test]
    fn test_is_git_uri_contains_gitlab_in_text() {
        assert!(is_git_uri("check gitlab.com/user/repo"));
    }

    #[test]
    fn test_is_git_uri_http_scheme() {
        assert!(is_git_uri("http://example.com/user/repo"));
    }
}
