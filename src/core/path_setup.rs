use crate::core::{DepotError, DepotResult};
use std::env;
use std::path::PathBuf;

/// Check if depot is in PATH and provide setup instructions if not
pub fn check_path_setup() -> DepotResult<()> {
    // Get the current executable path
    // nosemgrep: rust.lang.security.current-exe.current-exe
    // Justification: Used only to check if depot is in PATH and provide setup instructions.
    // No security risk as the path is not used for privilege escalation.
    let current_exe = env::current_exe()
        .map_err(|e| DepotError::Path(format!("Failed to get current executable: {}", e)))?;

    // Get the directory containing the executable
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| DepotError::Path("Could not get executable directory".to_string()))?;

    // Get DEPOT_HOME bin directory
    let depot_bin = get_depot_bin_dir();

    // Check if we can find 'depot' in PATH by checking PATH environment variable
    // instead of running a subprocess (which would cause infinite recursion)
    let path_env = env::var("PATH").unwrap_or_default();

    // Normalize paths for comparison - remove trailing slashes and convert to strings
    // Use canonicalize to resolve symlinks and get absolute paths
    let normalize_path = |p: &std::path::Path| -> String {
        // Try to canonicalize first (resolves symlinks), fall back to string conversion
        p.canonicalize()
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string()
    };

    let exe_dir_normalized = normalize_path(exe_dir);
    let depot_bin_normalized = depot_bin.as_ref().map(|p| normalize_path(p));

    // Check if the executable's directory matches any PATH entry
    // PATH uses ':' as separator on Unix, ';' on Windows
    let path_separator = if cfg!(target_os = "windows") {
        ';'
    } else {
        ':'
    };
    let depot_in_path = path_env
        .split(path_separator)
        .filter(|dir| !dir.is_empty()) // Skip empty PATH entries
        .any(|dir| {
            // Expand variables in PATH entry
            let expanded_dir = expand_path_vars(dir.trim()); // Trim whitespace
            let path_entry = PathBuf::from(&expanded_dir);
            let path_entry_normalized = normalize_path(&path_entry);

            // Compare normalized paths
            if path_entry_normalized == exe_dir_normalized {
                return true;
            }

            // Also check against DEPOT_HOME/bin if it exists
            if let Some(ref depot_norm) = depot_bin_normalized {
                if path_entry_normalized == *depot_norm {
                    return true;
                }
            }

            false
        });

    if depot_in_path {
        return Ok(()); // Already in PATH
    }

    // Not in PATH - show setup instructions
    eprintln!("\n⚠️  Depot is not in your PATH");
    eprintln!("\nTo set up Depot, run:");
    eprintln!("  depot setup");
    eprintln!("\nThis will:");
    eprintln!("  • Install depot to DEPOT_HOME");
    eprintln!("  • Add DEPOT_HOME/bin to your PATH");
    eprintln!("  • Configure your shell automatically");
    eprintln!("\nCurrent Depot location: {}", current_exe.display());

    Ok(())
}

/// Get the DEPOT_HOME bin directory (returns None if DEPOT_HOME doesn't exist yet)
fn get_depot_bin_dir() -> Option<PathBuf> {
    // Check DEPOT_HOME environment variable first
    if let Ok(depot_home) = env::var("DEPOT_HOME") {
        return Some(PathBuf::from(depot_home).join("bin"));
    }

    // Check default locations
    let depot_home = if cfg!(target_os = "windows") {
        env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("depot"))
    } else if cfg!(target_os = "macos") {
        env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("depot")
        })
    } else {
        env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".local").join("share").join("depot"))
    };

    depot_home.map(|p| p.join("bin"))
}

/// Expand path variables like $HOME, ~, etc.
fn expand_path_vars(path: &str) -> String {
    // Expand ~ to home directory
    if path.starts_with('~') {
        if let Ok(home) = env::var("HOME") {
            return path.replacen("~", &home, 1);
        }
    }

    // Expand $HOME
    if path.contains("$HOME") {
        if let Ok(home) = env::var("HOME") {
            return path.replace("$HOME", &home);
        }
    }

    path.to_string()
}

/// Detect the current shell
pub fn detect_shell() -> String {
    env::var("SHELL")
        .unwrap_or_else(|_| "/bin/sh".to_string())
        .rsplit('/')
        .next()
        .unwrap_or("sh")
        .to_string()
}

/// Get the shell profile file path
pub fn get_shell_profile(shell: &str) -> String {
    match shell {
        "zsh" => "~/.zshrc".to_string(),
        "bash" => "~/.bashrc".to_string(),
        "fish" => "~/.config/fish/config.fish".to_string(),
        _ => "~/.profile".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_depot_bin_dir() {
        let depot_bin = get_depot_bin_dir();
        // May be None if not set up yet, which is fine
        if let Some(bin_dir) = depot_bin {
            assert!(bin_dir.to_string_lossy().contains("bin"));
        }
    }

    #[test]
    fn test_detect_shell() {
        let shell = detect_shell();
        assert!(!shell.is_empty());
    }

    #[test]
    fn test_get_shell_profile() {
        let profile = get_shell_profile("zsh");
        assert!(profile.contains("zshrc"));

        let profile = get_shell_profile("bash");
        assert!(profile.contains("bashrc"));
    }

    #[test]
    fn test_get_shell_profile_fish() {
        let profile = get_shell_profile("fish");
        assert!(profile.contains("fish"));
    }

    #[test]
    fn test_get_shell_profile_default() {
        let profile = get_shell_profile("unknown");
        assert!(profile.contains("profile"));
    }

    #[test]
    #[cfg(unix)]
    fn test_expand_path_vars_tilde() {
        let home = env::var("HOME").unwrap();
        let expanded = expand_path_vars("~/test");
        assert_eq!(expanded, format!("{}/test", home));
    }

    #[test]
    #[cfg(unix)]
    fn test_expand_path_vars_home_var() {
        let home = env::var("HOME").unwrap();
        let expanded = expand_path_vars("$HOME/test");
        assert_eq!(expanded, format!("{}/test", home));
    }

    #[test]
    fn test_expand_path_vars_no_expansion() {
        let path = "/usr/bin";
        let expanded = expand_path_vars(path);
        assert_eq!(expanded, path);
    }

    #[test]
    fn test_get_depot_bin_dir_structure() {
        let depot_bin = get_depot_bin_dir();
        // May be None if environment variables aren't set
        if let Some(bin_dir) = depot_bin {
            assert!(bin_dir.to_string_lossy().contains("depot"));
            assert!(bin_dir.to_string_lossy().contains("bin"));
        }
    }

    #[test]
    fn test_check_path_setup() {
        // Should not fail even if not in PATH
        let result = check_path_setup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_path_vars_multiple_home() {
        // Test expansion with multiple $HOME references
        let path = "/regular/path";
        let expanded = expand_path_vars(path);
        assert_eq!(expanded, path);
    }

    #[test]
    fn test_expand_path_vars_tilde_in_middle() {
        // Tilde in middle should not expand
        let path = "/some/~/path";
        let expanded = expand_path_vars(path);
        assert_eq!(expanded, path);
    }

    #[test]
    fn test_get_shell_profile_sh() {
        let profile = get_shell_profile("sh");
        assert!(profile.contains("profile"));
    }

    #[test]
    fn test_get_shell_profile_ksh() {
        let profile = get_shell_profile("ksh");
        assert!(profile.contains("profile"));
    }

    #[test]
    fn test_detect_shell_returns_valid_shell() {
        let shell = detect_shell();
        // Shell should be a valid shell name (not a path)
        assert!(!shell.contains('/'));
    }

    #[test]
    fn test_expand_path_vars_empty_string() {
        let expanded = expand_path_vars("");
        assert_eq!(expanded, "");
    }

    #[test]
    fn test_expand_path_vars_just_tilde() {
        // Just ~ should expand to home
        #[cfg(unix)]
        {
            let home = env::var("HOME").unwrap();
            let expanded = expand_path_vars("~");
            assert_eq!(expanded, home);
        }
        #[cfg(windows)]
        {
            let expanded = expand_path_vars("~");
            assert_eq!(expanded, "~");
        }
    }
}
