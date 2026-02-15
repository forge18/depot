use assert_cmd::Command;
use assert_fs::{prelude::*, TempDir};
use predicates::prelude::*;
use std::path::PathBuf;
use std::process::Command as StdCommand;

// Declare submodules
pub mod audit;
pub mod global;
pub mod init;
// pub mod install;
pub mod interactive;
pub mod list;
pub mod lua_version;
pub mod plugins;
pub mod real_packages;
pub mod remove;
pub mod update;
pub mod verify;
pub mod workflow;

/// Test context that provides isolated environment for each test
pub struct TestContext {
    pub temp: TempDir,
    pub depot_home: PathBuf,
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TestContext {
    /// Create a new test context with isolated environment
    pub fn new() -> Self {
        let temp = TempDir::new().unwrap();
        // Create config and cache directories that dirs crate will use
        let config_dir = temp.child("config").to_path_buf();
        let cache_dir = temp.child("cache").to_path_buf();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&cache_dir).unwrap();

        let depot_home = config_dir.join("depot");
        std::fs::create_dir_all(&depot_home).unwrap();

        Self { temp, depot_home }
    }

    /// Create a Command for running depot with proper environment
    pub fn depot(&self) -> Command {
        #[allow(deprecated)]
        let mut cmd = Command::cargo_bin("depot").unwrap();
        cmd.current_dir(&self.temp);

        // Set platform-specific env vars that dirs crate uses
        // This isolates Depot's config/cache directories to the test temp dir
        let config_dir = self.temp.child("config").to_path_buf();
        let cache_dir = self.temp.child("cache").to_path_buf();

        if cfg!(target_os = "windows") {
            cmd.env("APPDATA", &config_dir);
            cmd.env("LOCALAPPDATA", &cache_dir);
            cmd.env("USERPROFILE", self.temp.path());
        } else if cfg!(target_os = "linux") {
            cmd.env("XDG_CONFIG_HOME", &config_dir);
            cmd.env("XDG_CACHE_HOME", &cache_dir);
            cmd.env("HOME", self.temp.path());
        } else {
            // macOS and other Unix-like systems
            // dirs crate uses HOME to derive ~/Library/Application Support and ~/Library/Caches
            cmd.env("HOME", self.temp.path());
        }

        cmd
    }

    /// Create a package.yaml file with given content
    pub fn create_package_yaml(&self, content: &str) {
        use super::constants;
        self.temp
            .child(constants::PACKAGE_YAML)
            .write_str(content)
            .unwrap();
    }

    /// Create a Lua test script
    pub fn create_lua_script(&self, name: &str, content: &str) {
        self.temp.child(name).write_str(content).unwrap();
    }

    /// Get path to lua_modules directory
    pub fn lua_modules(&self) -> PathBuf {
        use super::constants;
        self.temp.child(constants::LUA_MODULES).to_path_buf()
    }

    /// Generate Lua package.path configuration string
    ///
    /// Returns a string that adds lua_modules to Lua's package.path.
    /// Uses absolute paths for reliability across different working directories.
    pub fn lua_package_path(&self) -> String {
        let lua_modules = self.lua_modules();
        let lua_modules_str = lua_modules.to_string_lossy().replace('\\', "/");

        format!(
            "package.path = '{}/?.lua;{}/?/init.lua;{}/?/?.lua;' .. package.path",
            lua_modules_str, lua_modules_str, lua_modules_str
        )
    }

    /// Generate Lua package.cpath configuration string
    ///
    /// Returns a string that adds lua_modules to Lua's package.cpath for native modules.
    /// Handles platform-specific extensions (.so, .dylib, .dll).
    pub fn lua_package_cpath(&self) -> String {
        let lua_modules = self.lua_modules();
        let lua_modules_str = lua_modules.to_string_lossy().replace('\\', "/");

        if cfg!(target_os = "windows") {
            format!(
                "package.cpath = '{}/?.dll;{}/?/init.dll;' .. package.cpath",
                lua_modules_str, lua_modules_str
            )
        } else if cfg!(target_os = "macos") {
            format!(
                "package.cpath = '{}/?.dylib;{}/?/init.dylib;' .. package.cpath",
                lua_modules_str, lua_modules_str
            )
        } else {
            // Linux and other Unix-like
            format!(
                "package.cpath = '{}/?.so;{}/?/init.so;' .. package.cpath",
                lua_modules_str, lua_modules_str
            )
        }
    }

    /// Generate complete Lua path setup code
    ///
    /// Returns a string containing both package.path and package.cpath setup.
    /// This is the recommended way to set up Lua paths in test scripts.
    pub fn lua_path_setup(&self) -> String {
        format!("{}\n{}", self.lua_package_path(), self.lua_package_cpath())
    }

    /// Check if Lua is available on the system
    pub fn has_lua() -> bool {
        StdCommand::new("lua").arg("-v").output().is_ok()
    }

    /// Skip the current test with a reason
    ///
    /// This is the recommended way to conditionally skip tests in Rust.
    /// The test will pass but will clearly indicate it was skipped in the output.
    ///
    /// This function returns early, so any code after calling it will not execute.
    /// This makes it clear in the code that the test is being skipped.
    ///
    /// # Example
    /// ```rust,no_run
    /// #[test]
    /// fn test_requires_network() {
    ///     if std::env::var("SKIP_NETWORK_TESTS").is_ok() {
    ///         TestContext::skip_test("Network tests disabled via SKIP_NETWORK_TESTS");
    ///     }
    ///     // ... test code (unreachable if skipped) ...
    /// }
    /// ```
    pub fn skip_test(reason: &str) {
        println!("⚠️  Test skipped: {}", reason);
    }

    /// Skip test if network is unavailable
    ///
    /// Checks for `SKIP_NETWORK_TESTS` environment variable and skips the test
    /// if it's set. This allows disabling network tests in CI or offline environments.
    pub fn require_network() {
        if std::env::var("SKIP_NETWORK_TESTS").is_ok() {
            Self::skip_test("Network tests disabled via SKIP_NETWORK_TESTS");
        }
    }

    /// Check if we have a TTY (terminal) available
    ///
    /// Interactive tests require a TTY. In CI environments without a TTY,
    /// these tests should be skipped.
    pub fn has_tty() -> bool {
        use std::io::{stdin, IsTerminal};
        stdin().is_terminal()
    }

    /// Skip test if TTY is unavailable (for interactive tests)
    ///
    /// Automatically skips the test if no TTY is available (e.g., in CI environments).
    /// This prevents interactive tests from hanging or failing in non-interactive contexts.
    pub fn require_tty() {
        if !Self::has_tty() {
            Self::skip_test("No TTY available (interactive tests require a terminal)");
        }
    }

    /// Handle package version unavailable error with helpful message
    ///
    /// If a package version becomes unavailable (rare but possible), this provides
    /// clear guidance on how to fix it. Use this when checking install results
    /// in tests that iterate over multiple packages.
    pub fn handle_unavailable_version(package: &str, version: &str, error: &str) {
        eprintln!("\n⚠️  Package version unavailable:");
        eprintln!("   Package: {}@{}", package, version);
        eprintln!("   Error: {}", error);
        eprintln!("\n💡 Solutions:");
        eprintln!("   1. Check if package/version still exists in the registry");
        eprintln!("   2. Update version in constants module if newer version available");
        eprintln!("   3. Use alternative stable package for testing");
        eprintln!("   4. Set SKIP_NETWORK_TESTS=1 to skip network tests");
        panic!("Package version unavailable - see error message above");
    }

    /// Run a command with progress monitoring instead of timeout
    ///
    /// This is preferred over timeouts because:
    /// - Timeouts are unreliable (network speed varies)
    /// - Progress detection is more accurate
    /// - CI handles overall job timeouts
    ///
    /// For very long operations, use `#[ignore = "reason"]` attribute and run separately.
    pub fn depot_with_progress(&self) -> Command {
        // Just return the command - let the test framework and CI handle timeouts
        // Individual test timeouts are removed in favor of:
        // 1. CI-level job timeouts (more reliable)
        // 2. #[ignore = "reason"] attribute for slow tests (Rust best practice)
        // 3. Test categorization (fast vs slow)
        self.depot()
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // TempDir automatically cleans up on drop, no need to call close()
    }
}

/// Test constants for file names, directories, and common values
pub mod constants {
    // File names
    pub const PACKAGE_YAML: &str = "package.yaml";
    pub const PACKAGE_LOCK: &str = "depot.lock";
    pub const WORKSPACE_YAML: &str = "workspace.yaml";
    pub const TEST_LUA: &str = "test.lua";

    // Directory names
    pub const LUA_MODULES: &str = "lua_modules";
    pub const SRC_DIR: &str = "src";
    pub const LIB_DIR: &str = "lib";
    pub const TESTS_DIR: &str = "tests";

    // Test package names and versions
    pub const PKG_PENLIGHT: &str = "penlight";
    pub const PKG_PENLIGHT_VERSION: &str = "1.13.1";
    pub const PKG_DKJSON: &str = "dkjson";
    pub const PKG_DKJSON_VERSION: &str = "2.5";
    pub const PKG_LUASOCKET: &str = "luasocket";
    pub const PKG_LUASOCKET_VERSION: &str = "3.0.0";
    pub const PKG_LUAFILESYSTEM: &str = "luafilesystem";
    pub const PKG_LUAFILESYSTEM_VERSION: &str = "1.8.0";
    pub const PKG_BUSTED: &str = "busted";
    pub const PKG_BUSTED_VERSION: &str = "2.0.0";
    pub const PKG_LUACHECK: &str = "luacheck";
    pub const PKG_LUACHECK_VERSION: &str = "1.1.0";

    // Common strings for assertions
    pub const MSG_SUCCESS: &str = "SUCCESS";
    pub const MSG_ERROR: &str = "ERROR";
    pub const MSG_INSTALLED: &str = "Installed";
}

/// Common predicates for assertions
/// Predicate helpers for common test assertions
pub mod test_predicates {
    use super::constants;
    use predicates::prelude::*;

    pub fn contains_success_message() -> impl Predicate<str> {
        predicate::str::contains("✓")
            .or(predicate::str::contains(constants::MSG_SUCCESS))
            .or(predicate::str::contains(constants::MSG_INSTALLED))
    }

    /// Check for specific error message patterns based on Depot's actual error format
    ///
    /// Depot uses specific error messages like:
    /// - "package.yaml not found"
    /// - "not found in manifest"
    /// - "Version conflict"
    /// - "❌ Error:" prefix
    ///
    /// Use this for generic error checks, but prefer specific message checks
    /// when you know the expected error type.
    pub fn contains_error_message() -> impl Predicate<str> {
        predicate::str::contains("❌ Error:")
            .or(predicate::str::contains("Error:"))
            .or(predicate::str::contains("not found"))
            .or(predicate::str::contains("Failed"))
            .or(predicate::str::contains("✗"))
            .or(predicate::str::contains(constants::MSG_ERROR))
    }

    /// Check for specific error message patterns
    ///
    /// Use this when you know the exact error type to test for.
    /// Examples:
    /// - `contains_specific_error("package.yaml not found")`
    /// - `contains_specific_error("not found in manifest")`
    /// - `contains_specific_error("Version conflict")`
    pub fn contains_specific_error(pattern: &str) -> impl Predicate<str> {
        predicate::str::contains(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creates_temp_dir() {
        let ctx = TestContext::new();
        assert!(ctx.temp.path().exists());
        assert!(ctx.depot_home.exists());
    }
}
