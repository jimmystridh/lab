//! Directory entry loading and base score calculation.
//!
//! Handles reading the LAB_PATH directory, filtering hidden dirs and files,
//! detecting symlinks, computing mtime-based recency scores, and applying
//! date-prefix bonuses. Provides the `Entry` struct used throughout the app.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A directory entry in the labs directory.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Entry {
    /// The directory name (basename only).
    pub name: String,
    /// The full path to the directory. For symlinks, this is the resolved realpath.
    pub path: PathBuf,
    /// Whether this entry is a symbolic link.
    pub is_symlink: bool,
    /// The last modification time.
    pub mtime: SystemTime,
    /// The base score computed from recency and date-prefix bonus.
    pub base_score: f64,
}

/// Regex-like check for YYYY-MM-DD- date prefix pattern.
/// Returns true if the name starts with a date prefix like "2025-01-15-".
pub fn has_date_prefix(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 11 {
        return false;
    }
    // Check YYYY-MM-DD- pattern
    bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4] == b'-'
        && bytes[5].is_ascii_digit()
        && bytes[6].is_ascii_digit()
        && bytes[7] == b'-'
        && bytes[8].is_ascii_digit()
        && bytes[9].is_ascii_digit()
        && bytes[10] == b'-'
}

/// Compute the recency bonus: 3.0 / sqrt(hours_since_mtime + 1).
fn recency_bonus(mtime: SystemTime) -> f64 {
    let elapsed = SystemTime::now()
        .duration_since(mtime)
        .unwrap_or_default();
    let hours = elapsed.as_secs_f64() / 3600.0;
    3.0 / (hours + 1.0).sqrt()
}

/// Compute the base score for an entry:
///   base_score = 3.0 / sqrt(hours_since_mtime + 1) + (2.0 if date-prefixed)
fn compute_base_score(name: &str, mtime: SystemTime) -> f64 {
    let mut score = recency_bonus(mtime);
    if has_date_prefix(name) {
        score += 2.0;
    }
    score
}

/// Load all directory entries from the given base path.
///
/// Skips hidden directories (starting with '.'), regular files,
/// and entries that can't be accessed (ENOENT/EACCES handled silently).
/// Symlinks pointing to directories are included with `is_symlink = true`.
pub fn load_entries(base_path: &Path) -> Vec<Entry> {
    let read_dir = match fs::read_dir(base_path) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(), // ENOENT/EACCES: return empty
    };

    let mut entries = Vec::new();

    for dir_entry in read_dir {
        let dir_entry = match dir_entry {
            Ok(de) => de,
            Err(_) => continue, // Skip entries we can't read
        };

        let name = dir_entry.file_name().to_string_lossy().to_string();

        // Skip hidden directories (starting with '.')
        if name.starts_with('.') {
            continue;
        }

        let entry_path = dir_entry.path();

        // Check if this is a symlink
        let is_symlink = entry_path.symlink_metadata().is_ok_and(|m| m.is_symlink());

        // Get the stat info (follows symlinks)
        let metadata = match fs::metadata(&entry_path) {
            Ok(m) => m,
            Err(_) => continue, // ENOENT (broken symlink) or EACCES: skip silently
        };

        // Only include directories (including symlinks that resolve to directories)
        if !metadata.is_dir() {
            continue;
        }

        let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        // For symlinks, resolve to realpath; for regular dirs, use the path as-is
        let resolved_path = if is_symlink {
            fs::canonicalize(&entry_path).unwrap_or_else(|_| entry_path.clone())
        } else {
            entry_path
        };

        let base_score = compute_base_score(&name, mtime);

        entries.push(Entry {
            name,
            path: resolved_path,
            is_symlink,
            mtime,
            base_score,
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs as unix_fs;
    use std::time::{Duration, SystemTime};

    /// Create a temporary test directory with some subdirectories.
    fn setup_test_dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("create temp dir");
        // Create regular directories
        fs::create_dir(tmp.path().join("alpha")).unwrap();
        fs::create_dir(tmp.path().join("beta")).unwrap();
        fs::create_dir(tmp.path().join("2025-01-15-gamma")).unwrap();
        // Create a hidden directory (should be excluded)
        fs::create_dir(tmp.path().join(".hidden")).unwrap();
        // Create a regular file (should be excluded)
        fs::write(tmp.path().join("file.txt"), "hello").unwrap();
        tmp
    }

    #[test]
    fn test_has_date_prefix_valid() {
        assert!(has_date_prefix("2025-01-15-project"));
        assert!(has_date_prefix("2024-12-31-test"));
        assert!(has_date_prefix("1999-06-01-old"));
    }

    #[test]
    fn test_has_date_prefix_invalid() {
        assert!(!has_date_prefix("alpha"));
        assert!(!has_date_prefix("not-a-date"));
        assert!(!has_date_prefix("2025-1-1-short")); // month/day too short
        assert!(!has_date_prefix("2025-01-15")); // no trailing dash
        assert!(!has_date_prefix("20250115-nohyphen"));
        assert!(!has_date_prefix("")); // empty
        assert!(!has_date_prefix("short")); // too short
    }

    #[test]
    fn test_load_entries_basic() {
        let tmp = setup_test_dir();
        let entries = load_entries(tmp.path());

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"alpha"), "should contain alpha");
        assert!(names.contains(&"beta"), "should contain beta");
        assert!(names.contains(&"2025-01-15-gamma"), "should contain gamma");
    }

    #[test]
    fn test_load_entries_excludes_hidden() {
        let tmp = setup_test_dir();
        let entries = load_entries(tmp.path());

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&".hidden"), "should not contain .hidden");
    }

    #[test]
    fn test_load_entries_excludes_files() {
        let tmp = setup_test_dir();
        let entries = load_entries(tmp.path());

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"file.txt"), "should not contain regular files");
    }

    #[test]
    fn test_load_entries_symlinks_to_dirs() {
        let tmp = setup_test_dir();
        // Create a symlink to the alpha directory
        let link_path = tmp.path().join("alpha-link");
        unix_fs::symlink(tmp.path().join("alpha"), &link_path).unwrap();

        let entries = load_entries(tmp.path());
        let link_entry = entries.iter().find(|e| e.name == "alpha-link");

        assert!(link_entry.is_some(), "should contain alpha-link symlink");
        let entry = link_entry.unwrap();
        assert!(entry.is_symlink, "should be marked as symlink");
        // Resolved path should be the real path of alpha
        let expected_real = fs::canonicalize(tmp.path().join("alpha")).unwrap();
        assert_eq!(entry.path, expected_real, "symlink should resolve to realpath");
    }

    #[test]
    fn test_load_entries_symlinks_to_files_excluded() {
        let tmp = setup_test_dir();
        // Create a symlink to a file (should be excluded)
        let link_path = tmp.path().join("file-link");
        unix_fs::symlink(tmp.path().join("file.txt"), &link_path).unwrap();

        let entries = load_entries(tmp.path());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"file-link"), "symlinks to files should be excluded");
    }

    #[test]
    fn test_load_entries_nonexistent_dir() {
        let entries = load_entries(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(entries.is_empty(), "nonexistent dir should return empty");
    }

    #[test]
    fn test_date_prefix_bonus() {
        let now = SystemTime::now();
        let score_dated = compute_base_score("2025-01-15-project", now);
        let score_plain = compute_base_score("project", now);

        assert!(
            (score_dated - score_plain - 2.0).abs() < 0.01,
            "date prefix should add exactly +2.0: dated={}, plain={}, diff={}",
            score_dated,
            score_plain,
            score_dated - score_plain
        );
    }

    #[test]
    fn test_recency_bonus_fresh() {
        let now = SystemTime::now();
        let bonus = recency_bonus(now);
        // For a just-created entry, hours ≈ 0, so bonus ≈ 3.0/sqrt(1) = 3.0
        assert!(
            (bonus - 3.0).abs() < 0.1,
            "fresh entry should have recency bonus ≈ 3.0, got {}",
            bonus
        );
    }

    #[test]
    fn test_recency_bonus_one_hour() {
        let one_hour_ago = SystemTime::now() - Duration::from_secs(3600);
        let bonus = recency_bonus(one_hour_ago);
        // hours ≈ 1, so bonus ≈ 3.0/sqrt(2) ≈ 2.12
        assert!(
            (bonus - 2.12).abs() < 0.1,
            "1h old entry should have recency bonus ≈ 2.12, got {}",
            bonus
        );
    }

    #[test]
    fn test_recency_bonus_24_hours() {
        let day_ago = SystemTime::now() - Duration::from_secs(86400);
        let bonus = recency_bonus(day_ago);
        // hours ≈ 24, so bonus ≈ 3.0/sqrt(25) = 3.0/5 = 0.6
        assert!(
            (bonus - 0.6).abs() < 0.1,
            "24h old entry should have recency bonus ≈ 0.6, got {}",
            bonus
        );
    }

    #[test]
    fn test_recency_bonus_one_week() {
        let week_ago = SystemTime::now() - Duration::from_secs(7 * 86400);
        let bonus = recency_bonus(week_ago);
        // hours ≈ 168, so bonus ≈ 3.0/sqrt(169) = 3.0/13 ≈ 0.23
        assert!(
            (bonus - 0.23).abs() < 0.1,
            "1 week old entry should have recency bonus ≈ 0.23, got {}",
            bonus
        );
    }

    #[test]
    fn test_base_score_older_entries_score_lower() {
        let now = SystemTime::now();
        let one_hour_ago = now - Duration::from_secs(3600);

        let score_now = compute_base_score("test", now);
        let score_older = compute_base_score("test", one_hour_ago);

        assert!(
            score_now > score_older,
            "fresh entry ({}) should score higher than older ({})",
            score_now,
            score_older
        );
    }

    #[test]
    fn test_load_entries_empty_dir() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let entries = load_entries(tmp.path());
        assert!(entries.is_empty(), "empty dir should return empty vec");
    }

    #[test]
    fn test_entry_is_not_symlink_for_regular_dir() {
        let tmp = setup_test_dir();
        let entries = load_entries(tmp.path());
        let alpha = entries.iter().find(|e| e.name == "alpha").unwrap();
        assert!(!alpha.is_symlink, "regular dir should not be symlink");
    }

    #[test]
    fn test_broken_symlink_excluded() {
        let tmp = setup_test_dir();
        // Create a broken symlink
        let link_path = tmp.path().join("broken-link");
        unix_fs::symlink("/nonexistent/target", &link_path).unwrap();

        let entries = load_entries(tmp.path());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            !names.contains(&"broken-link"),
            "broken symlinks should be excluded"
        );
    }

    #[test]
    fn test_load_entries_path_is_correct_for_regular_dirs() {
        let tmp = setup_test_dir();
        let entries = load_entries(tmp.path());
        let alpha = entries.iter().find(|e| e.name == "alpha").unwrap();
        assert_eq!(alpha.path, tmp.path().join("alpha"));
    }
}
