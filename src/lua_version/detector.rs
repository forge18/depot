use crate::core::{DepotError, DepotResult};
use std::fmt;
use std::process::Command;
use std::str;

/// Represents a detected Lua version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LuaVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl fmt::Display for LuaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl LuaVersion {
    /// Create a new Lua version
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a version string from `lua -v` output
    ///
    /// Handles formats like:
    /// - "Lua 5.4.6"
    /// - "Lua 5.3.6"
    /// - "Lua 5.1.5"
    pub fn parse(version_str: &str) -> DepotResult<Self> {
        // Remove "Lua" prefix and whitespace
        let version_str = version_str.trim();
        let version_str = version_str
            .strip_prefix("Lua")
            .map(|s| s.trim())
            .unwrap_or(version_str);

        // Parse version parts (e.g., "5.4.6" -> major=5, minor=4, patch=6)
        let parts: Vec<&str> = version_str.split('.').collect();

        if parts.is_empty() {
            return Err(DepotError::Version(format!(
                "Invalid Lua version format: '{}'",
                version_str
            )));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| DepotError::Version(format!("Invalid major version: '{}'", parts[0])))?;

        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        // Accept any Lua version (5.x and potentially future versions)
        // No hardcoded restriction - let the ecosystem evolve
        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    /// Get version as string (e.g., "5.4.6")
    pub fn version_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Get major.minor version (e.g., "5.4")
    pub fn major_minor(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// Check if this is Lua 5.1
    pub fn is_5_1(&self) -> bool {
        self.major == 5 && self.minor == 1
    }

    /// Check if this is Lua 5.3
    pub fn is_5_3(&self) -> bool {
        self.major == 5 && self.minor == 3
    }

    /// Check if this is Lua 5.4
    pub fn is_5_4(&self) -> bool {
        self.major == 5 && self.minor == 4
    }

    /// Get mlua feature flag for this version
    pub fn mlua_feature(&self) -> &'static str {
        if self.is_5_1() {
            "lua51"
        } else if self.is_5_3() {
            "lua53"
        } else if self.is_5_4() {
            "lua54"
        } else {
            // Default to Lua 5.4 for unknown versions
            "lua54"
        }
    }

    /// Discover installed Lua versions by checking common locations
    ///
    /// This checks for:
    /// - lua (default Lua binary)
    /// - lua5.1, lua5.3, lua5.4 (versioned binaries)
    /// - luajit (LuaJIT, reports as Lua 5.1)
    ///
    /// Returns a sorted list of unique versions
    pub fn discover_installed() -> Vec<LuaVersion> {
        let mut versions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Common Lua binary names to check
        let commands = [
            "lua", "lua5.1", "lua5.2", "lua5.3", "lua5.4", "lua5.5", // Future-proof
            "luajit",
        ];

        for cmd in &commands {
            if let Ok(version) = LuaVersionDetector::detect_with_command(cmd) {
                let key = (version.major, version.minor);
                if seen.insert(key) {
                    versions.push(version);
                }
            }
        }

        // Sort versions (newest first)
        versions.sort_by(|a, b| b.cmp(a));
        versions
    }
}

/// Detects the installed Lua version
pub struct LuaVersionDetector;

impl LuaVersionDetector {
    /// Detect the installed Lua version by running `lua -v`
    pub fn detect() -> DepotResult<LuaVersion> {
        let output = Command::new("lua").arg("-v").output().map_err(|e| {
            DepotError::Version(format!("Failed to run 'lua -v': {}. Is Lua installed?", e))
        })?;

        if !output.status.success() {
            return Err(DepotError::Version(
                "Failed to get Lua version. 'lua -v' returned an error.".to_string(),
            ));
        }

        let stdout = str::from_utf8(&output.stdout).map_err(|e| {
            DepotError::Version(format!("Invalid UTF-8 in Lua version output: {}", e))
        })?;

        // Parse first line (version info is usually on first line)
        let first_line = stdout.lines().next().unwrap_or("").trim();
        LuaVersion::parse(first_line)
    }

    /// Detect Lua version with a specific command
    ///
    /// Useful for testing or when Lua is installed with a different name
    pub fn detect_with_command(command: &str) -> DepotResult<LuaVersion> {
        let output = Command::new(command)
            .arg("-v")
            .output()
            .map_err(|e| DepotError::Version(format!("Failed to run '{} -v': {}", command, e)))?;

        if !output.status.success() {
            return Err(DepotError::Version(format!(
                "Failed to get Lua version from '{}'",
                command
            )));
        }

        let stdout = str::from_utf8(&output.stdout).map_err(|e| {
            DepotError::Version(format!("Invalid UTF-8 in Lua version output: {}", e))
        })?;

        let first_line = stdout.lines().next().unwrap_or("").trim();
        LuaVersion::parse(first_line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let v = LuaVersion::parse("Lua 5.4.6").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 4);
        assert_eq!(v.patch, 6);
        assert!(v.is_5_4());
    }

    #[test]
    fn test_parse_version_5_3() {
        let v = LuaVersion::parse("Lua 5.3.6").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 3);
        assert_eq!(v.patch, 6);
        assert!(v.is_5_3());
    }

    #[test]
    fn test_parse_version_5_1() {
        let v = LuaVersion::parse("Lua 5.1.5").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 5);
        assert!(v.is_5_1());
    }

    #[test]
    fn test_parse_version_without_lua_prefix() {
        let v = LuaVersion::parse("5.4.6").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 4);
        assert_eq!(v.patch, 6);
    }

    #[test]
    fn test_mlua_feature() {
        assert_eq!(LuaVersion::new(5, 1, 0).mlua_feature(), "lua51");
        assert_eq!(LuaVersion::new(5, 3, 0).mlua_feature(), "lua53");
        assert_eq!(LuaVersion::new(5, 4, 0).mlua_feature(), "lua54");
    }

    #[test]
    fn test_any_version_accepted() {
        // Now accepts any Lua version, not just 5.1, 5.3, 5.4
        let v52 = LuaVersion::parse("Lua 5.2.4").unwrap();
        assert_eq!(v52.major, 5);
        assert_eq!(v52.minor, 2);
        assert_eq!(v52.patch, 4);

        let v60 = LuaVersion::parse("Lua 6.0.0").unwrap();
        assert_eq!(v60.major, 6);
        assert_eq!(v60.minor, 0);
        assert_eq!(v60.patch, 0);
    }

    #[test]
    fn test_version_string() {
        let v = LuaVersion::new(5, 4, 6);
        assert_eq!(v.version_string(), "5.4.6");
    }

    #[test]
    fn test_major_minor() {
        let v = LuaVersion::new(5, 4, 6);
        assert_eq!(v.major_minor(), "5.4");
    }

    #[test]
    fn test_parse_version_minor_only() {
        let v = LuaVersion::parse("5.4").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 4);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_parse_version_major_only() {
        // Now accepts any valid version format, including just major version
        let v = LuaVersion::parse("5").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(LuaVersion::parse("").is_err());
        assert!(LuaVersion::parse("invalid").is_err());
    }

    #[test]
    fn test_display() {
        let v = LuaVersion::new(5, 4, 6);
        assert_eq!(format!("{}", v), "5.4.6");
    }

    #[test]
    fn test_parse_with_whitespace() {
        let v = LuaVersion::parse("  Lua 5.4.6  ").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 4);
        assert_eq!(v.patch, 6);
    }

    #[test]
    fn test_discover_installed() {
        // This test may find different versions depending on what's installed
        // Just verify it returns a vec and doesn't panic
        let versions = LuaVersion::discover_installed();

        // If versions are found, they should be sorted (newest first)
        for i in 1..versions.len() {
            assert!(
                versions[i - 1] >= versions[i],
                "Versions should be sorted newest first"
            );
        }
    }

    #[test]
    fn test_discover_installed_uniqueness() {
        let versions = LuaVersion::discover_installed();
        let mut seen = std::collections::HashSet::new();
        for v in &versions {
            let key = (v.major, v.minor);
            assert!(seen.insert(key), "Should not have duplicate versions");
        }
    }
}
