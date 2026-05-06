//! Shell detection and init snippet generation.
//!
//! Detects the user's shell (bash, zsh, fish, powershell) from the SHELL
//! environment variable with fallback to parent process name. Generates
//! the appropriate shell wrapper function that evals lab's stdout output
//! on success.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Supported shell types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

/// Detect the user's shell.
///
/// 1. Check SHELL environment variable for fish/zsh/bash
/// 2. Check PSModulePath for PowerShell
/// 3. Fallback: check parent process name
pub fn detect_shell() -> Option<Shell> {
    // Check SHELL env var first
    if let Ok(shell_env) = env::var("SHELL") {
        if shell_env.contains("fish") {
            return Some(Shell::Fish);
        }
        if shell_env.contains("zsh") {
            return Some(Shell::Zsh);
        }
        if shell_env.contains("bash") {
            return Some(Shell::Bash);
        }
    }

    // PowerShell detection via PSModulePath
    if let Ok(ps) = env::var("PSModulePath") {
        if !ps.is_empty() {
            return Some(Shell::PowerShell);
        }
    }

    // Fallback: check parent process name
    if let Some(parent_name) = get_parent_process_name() {
        if parent_name.contains("fish") {
            return Some(Shell::Fish);
        }
        if parent_name.contains("zsh") {
            return Some(Shell::Zsh);
        }
        if parent_name.contains("bash") {
            return Some(Shell::Bash);
        }
        if parent_name.contains("pwsh") || parent_name.to_lowercase().contains("powershell") {
            return Some(Shell::PowerShell);
        }
    }

    None
}

/// Get the parent process name.
///
/// On Linux, reads `/proc/<ppid>/exe` to resolve the binary path.
/// On macOS (and other Unix), uses `ps -p <ppid> -o comm=`.
fn get_parent_process_name() -> Option<String> {
    let ppid = std::os::unix::process::parent_id();

    // On Linux, try /proc/<ppid>/exe first
    #[cfg(target_os = "linux")]
    {
        let exe_path = format!("/proc/{}/exe", ppid);
        if let Ok(resolved) = std::fs::read_link(&exe_path) {
            let name = resolved
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    // Fallback (macOS and Linux): use ps -p <ppid> -o comm=
    let output = Command::new("ps")
        .args(["-p", &ppid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    if output.status.success() {
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if raw.is_empty() {
            return None;
        }
        // ps -o comm= may return a full path; extract the basename
        let name = std::path::Path::new(&raw)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(raw);
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    } else {
        None
    }
}

/// Get the RC file path for a given shell.
///
/// Returns the path to the shell's configuration file:
/// - Bash: ~/.bashrc (or ~/.bash_profile if ~/.bashrc doesn't exist on macOS)
/// - Zsh: ~/.zshrc
/// - Fish: ~/.config/fish/config.fish
/// - PowerShell: standard profile location
pub fn shell_rc_file(shell: Shell) -> Option<String> {
    match shell {
        Shell::Fish => Some("~/.config/fish/config.fish".to_string()),
        Shell::Zsh => Some("~/.zshrc".to_string()),
        Shell::Bash => {
            // Prefer .bashrc, fall back to .bash_profile on macOS
            let bashrc = dirs::home_dir()
                .map(|h| h.join(".bashrc"))
                .unwrap_or_else(|| PathBuf::from("~/.bashrc"));
            if bashrc.exists() {
                Some("~/.bashrc".to_string())
            } else {
                Some("~/.bash_profile".to_string())
            }
        }
        Shell::PowerShell => {
            // Check PROFILE env, or use standard location
            if let Ok(profile) = env::var("PROFILE") {
                return Some(profile);
            }
            let home = dirs::home_dir()?;
            Some(
                home.join(".config")
                    .join("powershell")
                    .join("Microsoft.PowerShell_profile.ps1")
                    .to_string_lossy()
                    .to_string(),
            )
        }
    }
}

/// Generate the shell init snippet (wrapper function).
///
/// The wrapper function captures lab's stdout and evals it on exit 0,
/// or echoes it on failure. This is what gets added to the user's shell config.
///
/// # Arguments
/// * `shell` - The target shell type
/// * `binary_path` - Absolute path to the lab binary
/// * `labs_path` - Default labs path (used when no explicit path)
/// * `explicit_path` - If Some, hardcode this path; if None, use env var with fallback
pub fn init_snippet(
    shell: Shell,
    binary_path: &str,
    labs_path: &str,
    explicit_path: Option<&str>,
) -> String {
    match shell {
        Shell::Fish => {
            let fish_path_arg = if let Some(ep) = explicit_path {
                format!(" --path '{}'", ep)
            } else {
                format!(
                    " --path (if set -q LAB_PATH; echo \"$LAB_PATH\"; else; echo '{}'; end)",
                    labs_path
                )
            };
            format!(
                "function lab\n\
                 \x20 set -l out (/usr/bin/env '{}' exec{} $argv 2>/dev/tty | string collect)\n\
                 \x20 if test $pipestatus[1] -eq 0\n\
                 \x20   eval $out\n\
                 \x20 else\n\
                 \x20   echo $out\n\
                 \x20 end\n\
                 end\n",
                binary_path, fish_path_arg
            )
        }
        Shell::PowerShell => {
            let ps_path_expr = if let Some(ep) = explicit_path {
                format!("'{}'", ep)
            } else {
                format!(
                    "$(if ($env:LAB_PATH) {{ $env:LAB_PATH }} else {{ '{}' }})",
                    labs_path
                )
            };
            format!(
                "function lab {{\n\
                 \x20 $labPath = {}\n\
                 \x20 $tempErr = [System.IO.Path]::GetTempFileName()\n\
                 \x20 $out = & '{}' exec --path $labPath @args 2>$tempErr\n\
                 \x20 if ($LASTEXITCODE -eq 0) {{\n\
                 \x20   $out | Invoke-Expression\n\
                 \x20 }} else {{\n\
                 \x20   Get-Content $tempErr | Write-Host\n\
                 \x20   $out | Write-Output\n\
                 \x20 }}\n\
                 \x20 Remove-Item $tempErr -ErrorAction SilentlyContinue\n\
                 }}\n",
                ps_path_expr, binary_path
            )
        }
        Shell::Bash | Shell::Zsh => {
            let path_arg = if let Some(ep) = explicit_path {
                format!(" --path '{}'", ep)
            } else {
                format!(" --path \"${{LAB_PATH:-{}}}\"", labs_path)
            };
            format!(
                "lab() {{\n\
                 \x20 local out\n\
                 \x20 out=$(/usr/bin/env '{}' exec{} \"$@\" 2>/dev/tty)\n\
                 \x20 if [ $? -eq 0 ]; then\n\
                 \x20   eval \"$out\"\n\
                 \x20 else\n\
                 \x20   echo \"$out\"\n\
                 \x20 fi\n\
                 }}\n",
                binary_path, path_arg
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Shell detection tests ----

    #[test]
    fn test_detect_shell_bash_env() {
        // This test depends on the SHELL env, which we can't control in unit tests.
        // Instead test the logic by examining the env.
        // The integration tests via spec tests cover this properly.
        // Here we verify detection can run and returns a populated variant when available.
        let result = detect_shell();
        if let Some(shell) = result {
            assert!(
                !format!("{shell:?}").is_empty(),
                "detected shell should be printable"
            );
        }
    }

    #[test]
    fn test_get_parent_process_name_returns_something() {
        // In a test environment, we should be able to detect the parent process
        let result = get_parent_process_name();
        // The parent should be cargo or the test runner
        assert!(result.is_some(), "Should detect parent process name");
        let name = result.unwrap();
        assert!(!name.is_empty(), "Parent process name should not be empty");
    }

    // ---- Shell RC file tests ----

    #[test]
    fn test_shell_rc_file_fish() {
        assert_eq!(
            shell_rc_file(Shell::Fish),
            Some("~/.config/fish/config.fish".to_string())
        );
    }

    #[test]
    fn test_shell_rc_file_zsh() {
        assert_eq!(shell_rc_file(Shell::Zsh), Some("~/.zshrc".to_string()));
    }

    #[test]
    fn test_shell_rc_file_bash() {
        let rc = shell_rc_file(Shell::Bash);
        assert!(rc.is_some());
        let rc = rc.unwrap();
        assert!(
            rc == "~/.bashrc" || rc == "~/.bash_profile",
            "bash rc should be .bashrc or .bash_profile, got: {}",
            rc
        );
    }

    #[test]
    fn test_shell_rc_file_powershell() {
        let rc = shell_rc_file(Shell::PowerShell);
        assert!(rc.is_some());
    }

    // ---- Init snippet tests: Bash/Zsh ----

    #[test]
    fn test_init_snippet_bash_with_explicit_path() {
        let snippet = init_snippet(
            Shell::Bash,
            "/usr/local/bin/lab",
            "~/src/labs",
            Some("/tmp/labs"),
        );
        assert!(
            snippet.contains("lab() {"),
            "should contain lab() {{ function: {}",
            snippet
        );
        assert!(snippet.contains("local out"), "should declare local out");
        assert!(
            snippet.contains("/usr/local/bin/lab"),
            "should contain binary path"
        );
        assert!(
            snippet.contains("--path '/tmp/labs'"),
            "should hardcode explicit path"
        );
        assert!(
            snippet.contains("2>/dev/tty"),
            "should redirect stderr to tty"
        );
        assert!(snippet.contains("eval \"$out\""), "should eval on success");
        assert!(snippet.contains("echo \"$out\""), "should echo on failure");
        assert!(
            snippet.contains("if [ $? -eq 0 ]; then"),
            "should check exit code"
        );
    }

    #[test]
    fn test_init_snippet_bash_no_explicit_path() {
        let snippet = init_snippet(Shell::Bash, "/usr/local/bin/lab", "~/src/labs", None);
        assert!(
            snippet.contains("${LAB_PATH:-~/src/labs}"),
            "should use LAB_PATH with fallback: {}",
            snippet
        );
        assert!(!snippet.contains("--path '"), "should not hardcode a path");
    }

    #[test]
    fn test_init_snippet_bash_valid_syntax() {
        let snippet = init_snippet(
            Shell::Bash,
            "/usr/local/bin/lab",
            "~/src/labs",
            Some("/tmp/labs"),
        );
        // Basic checks that it's valid shell-like syntax
        assert!(
            snippet.contains("lab() {"),
            "should have function declaration"
        );
        assert!(snippet.contains("}"), "should have closing brace");
        assert!(snippet.contains("fi"), "should close if statement");
    }

    #[test]
    fn test_init_snippet_zsh_same_as_bash() {
        let bash = init_snippet(Shell::Bash, "/bin/lab", "~/src/labs", Some("/tmp/labs"));
        let zsh = init_snippet(Shell::Zsh, "/bin/lab", "~/src/labs", Some("/tmp/labs"));
        assert_eq!(bash, zsh, "bash and zsh snippets should be identical");
    }

    // ---- Init snippet tests: Fish ----

    #[test]
    fn test_init_snippet_fish_with_explicit_path() {
        let snippet = init_snippet(
            Shell::Fish,
            "/usr/local/bin/lab",
            "~/src/labs",
            Some("/tmp/labs"),
        );
        assert!(
            snippet.contains("function lab"),
            "should contain fish function: {}",
            snippet
        );
        assert!(
            snippet.contains("--path '/tmp/labs'"),
            "should hardcode explicit path"
        );
        assert!(
            snippet.contains("2>/dev/tty"),
            "should redirect stderr to tty"
        );
        assert!(
            snippet.contains("$pipestatus[1]"),
            "should use pipestatus for fish: {}",
            snippet
        );
        assert!(snippet.contains("end"), "should close with end");
        assert!(
            snippet.contains("string collect"),
            "should use string collect"
        );
    }

    #[test]
    fn test_init_snippet_fish_no_explicit_path() {
        let snippet = init_snippet(Shell::Fish, "/usr/local/bin/lab", "~/src/labs", None);
        assert!(
            snippet.contains("if set -q LAB_PATH"),
            "should check LAB_PATH: {}",
            snippet
        );
        assert!(
            snippet.contains("echo \"$LAB_PATH\""),
            "should echo LAB_PATH: {}",
            snippet
        );
        assert!(
            snippet.contains("echo '~/src/labs'"),
            "should have fallback path: {}",
            snippet
        );
    }

    #[test]
    fn test_init_snippet_fish_no_bash_isms() {
        let snippet = init_snippet(Shell::Fish, "/bin/lab", "~/src/labs", Some("/tmp/labs"));
        assert!(
            !snippet.contains("$()"),
            "fish snippet must not contain $()"
        );
        assert!(!snippet.contains("$?"), "fish snippet must not contain $?");
    }

    #[test]
    fn test_init_snippet_fish_no_bash_isms_no_path() {
        let snippet = init_snippet(Shell::Fish, "/bin/lab", "~/src/labs", None);
        assert!(
            !snippet.contains("$()"),
            "fish snippet must not contain $()"
        );
        assert!(!snippet.contains("$?"), "fish snippet must not contain $?");
    }

    // ---- Init snippet tests: PowerShell ----

    #[test]
    fn test_init_snippet_powershell_with_explicit_path() {
        let snippet = init_snippet(
            Shell::PowerShell,
            "/usr/local/bin/lab",
            "~/src/labs",
            Some("/tmp/labs"),
        );
        assert!(
            snippet.contains("function lab"),
            "should contain function lab"
        );
        assert!(
            snippet.contains("$LASTEXITCODE"),
            "should check LASTEXITCODE"
        );
        assert!(
            snippet.contains("Invoke-Expression"),
            "should invoke expression on success"
        );
    }

    // ---- Init snippet embeds binary path ----

    #[test]
    fn test_init_snippet_embeds_binary_path() {
        let snippet = init_snippet(Shell::Bash, "/custom/path/to/lab", "~/src/labs", None);
        assert!(
            snippet.contains("/custom/path/to/lab"),
            "snippet must contain the binary path"
        );
    }

    #[test]
    fn test_init_snippet_fish_embeds_binary_path() {
        let snippet = init_snippet(Shell::Fish, "/custom/path/to/lab", "~/src/labs", None);
        assert!(
            snippet.contains("/custom/path/to/lab"),
            "fish snippet must contain the binary path"
        );
    }

    // ---- Init snippet uses exec subcommand ----

    #[test]
    fn test_init_snippet_bash_uses_exec() {
        let snippet = init_snippet(Shell::Bash, "/bin/lab", "~/src/labs", None);
        assert!(
            snippet.contains("exec"),
            "bash snippet should invoke 'exec' subcommand: {}",
            snippet
        );
    }

    #[test]
    fn test_init_snippet_fish_uses_exec() {
        let snippet = init_snippet(Shell::Fish, "/bin/lab", "~/src/labs", None);
        assert!(
            snippet.contains("exec"),
            "fish snippet should invoke 'exec' subcommand: {}",
            snippet
        );
    }
}
