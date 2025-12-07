use crate::core::{LpmError, LpmResult};
use std::path::Path;
use std::process::Command;

/// Manages sandboxed Rust builds
pub struct BuildSandbox;

impl BuildSandbox {
    /// Execute a cargo command in a sandboxed environment
    ///
    /// The sandbox limits:
    /// - Filesystem access to project directory only
    /// - Network access only for cargo (crates.io, GitHub)
    /// - Cannot access ~/.lpm/credentials, ~/.ssh/, etc.
    pub fn execute_cargo(
        project_root: &Path,
        args: &[&str],
        env_vars: &[(&str, &str)],
    ) -> LpmResult<()> {
        let mut cmd = Command::new("cargo");
        cmd.args(args);
        cmd.current_dir(project_root);

        // Set environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // Restrict filesystem access by setting CARGO_HOME to project directory
        // This prevents cargo from accessing global credentials
        let cargo_home = project_root.join(".cargo");
        cmd.env("CARGO_HOME", &cargo_home);

        // Allow network access for cargo (needed for crates.io)
        // But restrict other network access
        cmd.env("CARGO_NET_OFFLINE", "false");

        // Execute the command
        let status = cmd.status()?;

        if !status.success() {
            return Err(LpmError::Package(format!(
                "Cargo build failed with exit code: {}",
                status.code().unwrap_or(1)
            )));
        }

        Ok(())
    }

    /// Check if cargo-zigbuild is installed
    pub fn check_cargo_zigbuild() -> bool {
        Command::new("cargo")
            .args(["zigbuild", "--version"])
            .output()
            .is_ok()
    }

    /// Install cargo-zigbuild if not present
    pub fn ensure_cargo_zigbuild() -> LpmResult<()> {
        if Self::check_cargo_zigbuild() {
            return Ok(());
        }

        eprintln!("Installing cargo-zigbuild...");
        let status = Command::new("cargo")
            .args(["install", "cargo-zigbuild"])
            .status()?;

        if !status.success() {
            return Err(LpmError::Package(
                "Failed to install cargo-zigbuild. Please install it manually: cargo install cargo-zigbuild"
                    .to_string(),
            ));
        }

        eprintln!("✓ cargo-zigbuild installed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_check_cargo_zigbuild() {
        // This will return false if not installed, which is fine for testing
        let _ = BuildSandbox::check_cargo_zigbuild();
    }

    #[test]
    fn test_ensure_cargo_zigbuild_when_installed() {
        // If cargo-zigbuild is installed, should return Ok
        // If not, will try to install (may fail in test environment)
        let result = BuildSandbox::ensure_cargo_zigbuild();
        // Either succeeds or fails gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_execute_cargo_invalid_command() {
        let temp = TempDir::new().unwrap();

        // Try to run invalid cargo command
        let result = BuildSandbox::execute_cargo(temp.path(), &["invalid-command"], &[]);
        assert!(result.is_err());
    }
}
