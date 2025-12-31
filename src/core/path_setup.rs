use crate::core::{LpmError, LpmResult};
use std::env;
use std::path::PathBuf;

/// Check if lpm is in PATH and provide setup instructions if not
pub fn check_path_setup() -> LpmResult<()> {
    // Get the current executable path
    let current_exe = env::current_exe()
        .map_err(|e| LpmError::Path(format!("Failed to get current executable: {}", e)))?;

    // Get the directory containing the executable
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| LpmError::Path("Could not get executable directory".to_string()))?;

    // Get cargo bin directory
    let cargo_bin = get_cargo_bin_dir()?;

    // Check if we can find 'lpm' in PATH by checking PATH environment variable
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
    let cargo_bin_normalized = normalize_path(&cargo_bin);

    // Check if the executable's directory matches any PATH entry
    // PATH uses ':' as separator on Unix, ';' on Windows
    let path_separator = if cfg!(target_os = "windows") {
        ';'
    } else {
        ':'
    };
    let lpm_in_path = path_env
        .split(path_separator)
        .filter(|dir| !dir.is_empty()) // Skip empty PATH entries
        .any(|dir| {
            // Expand variables in PATH entry
            let expanded_dir = expand_path_vars(dir.trim()); // Trim whitespace
            let path_entry = PathBuf::from(&expanded_dir);
            let path_entry_normalized = normalize_path(&path_entry);

            // Compare normalized paths
            path_entry_normalized == exe_dir_normalized
                || path_entry_normalized == cargo_bin_normalized
        });

    if lpm_in_path {
        return Ok(()); // Already in PATH
    }

    // Check if we're in a common cargo bin location
    if exe_dir_normalized == cargo_bin_normalized || current_exe.starts_with(&cargo_bin) {
        // We're installed via cargo, but not in PATH
        eprintln!("\n⚠️  LPM is not in your PATH");
        eprintln!("\nTo add LPM to your PATH:");

        if cfg!(target_os = "windows") {
            eprintln!("  1. Open System Properties > Environment Variables");
            eprintln!("  2. Add {} to your PATH", cargo_bin.display());
            eprintln!("\nOr run this in PowerShell (as Administrator):");
            eprintln!(
                "  [Environment]::SetEnvironmentVariable(\"Path\", $env:Path + \";{}\", \"User\")",
                cargo_bin.display()
            );
        } else {
            // Unix-like systems
            let shell = detect_shell();
            let profile_file = get_shell_profile(&shell);

            eprintln!("  Add this line to your {}:", profile_file);
            eprintln!("  export PATH=\"$HOME/.cargo/bin:$PATH\"");
            eprintln!("\nOr run this command:");
            eprintln!(
                "  echo 'export PATH=\"$HOME/.cargo/bin:$PATH\"' >> {}",
                profile_file
            );
            eprintln!("\nThen reload your shell:");
            eprintln!("  source {}", profile_file);
        }

        eprintln!("\nCurrent LPM location: {}", current_exe.display());
    }

    Ok(())
}

/// Get the cargo bin directory
fn get_cargo_bin_dir() -> LpmResult<PathBuf> {
    if cfg!(target_os = "windows") {
        // Windows: %USERPROFILE%\.cargo\bin
        let userprofile = env::var("USERPROFILE")
            .map_err(|_| LpmError::Path("USERPROFILE not set".to_string()))?;
        Ok(PathBuf::from(userprofile).join(".cargo").join("bin"))
    } else {
        // Unix: ~/.cargo/bin
        let home = env::var("HOME").map_err(|_| LpmError::Path("HOME not set".to_string()))?;
        Ok(PathBuf::from(home).join(".cargo").join("bin"))
    }
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
fn detect_shell() -> String {
    env::var("SHELL")
        .unwrap_or_else(|_| "/bin/sh".to_string())
        .rsplit('/')
        .next()
        .unwrap_or("sh")
        .to_string()
}

/// Get the shell profile file path
fn get_shell_profile(shell: &str) -> String {
    match shell {
        "zsh" => "~/.zshrc".to_string(),
        "bash" => "~/.bashrc".to_string(),
        "fish" => "~/.config/fish/config.fish".to_string(),
        _ => "~/.profile".to_string(),
    }
}

/// Attempt to automatically add cargo bin to PATH (Unix only)
///
/// This modifies the user's shell profile file. Use with caution.
pub fn setup_path_auto() -> LpmResult<()> {
    if cfg!(target_os = "windows") {
        return Err(LpmError::Package(
            "Automatic PATH setup not supported on Windows. Please add manually.".to_string(),
        ));
    }

    // Verify cargo bin directory exists (for error checking)
    get_cargo_bin_dir()?;
    let shell = detect_shell();
    let profile_file = get_shell_profile(&shell);

    // Expand ~ to home directory
    let home = env::var("HOME").map_err(|_| LpmError::Path("HOME not set".to_string()))?;
    let profile_path = profile_file.replace("~", &home);

    // Check if PATH is already set
    use std::fs;
    let profile_content = fs::read_to_string(&profile_path).unwrap_or_else(|_| String::new());

    if profile_content.contains("$HOME/.cargo/bin") || profile_content.contains(".cargo/bin") {
        println!("✓ PATH already configured in {}", profile_file);
        return Ok(());
    }

    // Add PATH export
    let path_line = "\nexport PATH=\"$HOME/.cargo/bin:$PATH\"\n";

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&profile_path)
        .map_err(|e| LpmError::Path(format!("Failed to open {}: {}", profile_file, e)))?;

    file.write_all(path_line.as_bytes())
        .map_err(|e| LpmError::Path(format!("Failed to write to {}: {}", profile_file, e)))?;

    println!("✓ Added LPM to PATH in {}", profile_file);
    println!("  Run 'source {}' to apply changes", profile_file);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cargo_bin_dir() {
        let cargo_bin = get_cargo_bin_dir().unwrap();
        assert!(cargo_bin.to_string_lossy().contains(".cargo"));
        assert!(cargo_bin.to_string_lossy().contains("bin"));
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
    fn test_get_cargo_bin_dir_windows() {
        // This test will work on both platforms, just checks the structure
        let cargo_bin = get_cargo_bin_dir().unwrap();
        assert!(cargo_bin.to_string_lossy().contains("cargo"));
        assert!(cargo_bin.to_string_lossy().contains("bin"));
    }
}
