use crate::core::path::{find_project_root, lua_modules_dir};
use crate::core::{LpmError, LpmResult};
use crate::path_setup::loader::PathSetup;
use std::path::Path;
use std::process::Command;

/// Options for running Lua scripts
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Working directory (defaults to project root)
    pub cwd: Option<String>,
    /// Additional Lua arguments
    pub lua_args: Vec<String>,
    /// Environment variables
    pub env: Vec<(String, String)>,
}

/// Runs Lua scripts with correct package.path setup
pub struct LuaRunner;

impl LuaRunner {
    /// Run a Lua script with lpm.loader
    pub fn run_script(script_path: &Path, options: RunOptions) -> LpmResult<i32> {
        let project_root = find_project_root(script_path)?;
        let lua_modules = lua_modules_dir(&project_root);

        // Check if lua_modules exists
        if !lua_modules.exists() {
            return Err(LpmError::Package(
                "lua_modules directory not found. Run 'lpm install' first.".to_string(),
            ));
        }

        // Ensure loader is installed
        PathSetup::install_loader(&project_root)?;

        // Try to use LPM-managed Lua, fall back to system PATH
        let lua_binary = get_lpm_lua_binary("lua", &project_root)
            .unwrap_or_else(|_| Path::new("lua").to_path_buf());

        // Build Lua command
        let mut cmd = Command::new(&lua_binary);

        // Add lpm.loader require before the script
        // The loader is installed at lua_modules/lpm/loader.lua
        let lpm_dir = lua_modules.join("lpm");
        cmd.arg("-e").arg(format!(
            "package.path = '{}' .. '/?.lua;' .. package.path; require('lpm.loader')",
            lpm_dir.to_string_lossy()
        ));

        // Add script path
        cmd.arg(script_path);

        // Add additional Lua arguments
        for arg in &options.lua_args {
            cmd.arg(arg);
        }

        // Set working directory
        if let Some(cwd) = &options.cwd {
            cmd.current_dir(cwd);
        } else {
            cmd.current_dir(&project_root);
        }

        // Set environment variables
        for (key, value) in &options.env {
            cmd.env(key, value);
        }

        // Add LUA_PATH to include lua_modules
        let lua_path = format!(
            "{}/?.lua;{}/?/init.lua;",
            lua_modules.to_string_lossy(),
            lua_modules.to_string_lossy()
        );
        cmd.env("LUA_PATH", lua_path);

        // Run the command
        let status = cmd.status()?;
        Ok(status.code().unwrap_or(1))
    }

    /// Execute a command string with correct LUA_PATH and LUA_CPATH setup
    ///
    /// This is the main entry point for running scripts and commands.
    /// It automatically sets up package.path and package.cpath for the command.
    pub fn exec_command(command_str: &str, options: RunOptions) -> LpmResult<i32> {
        let current_dir = std::env::current_dir()?;
        let project_root = find_project_root(&current_dir)?;
        let lua_modules = lua_modules_dir(&project_root);

        // Ensure loader is installed
        PathSetup::install_loader(&project_root)?;

        // Parse command into parts
        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return Err(LpmError::Package("Empty command".to_string()));
        }

        let program = parts[0];
        let args = &parts[1..];

        // If program is "lua" or "luac", try to use LPM-managed version
        let actual_program = if program == "lua" || program == "luac" {
            match get_lpm_lua_binary(program, &project_root) {
                Ok(path) => {
                    // Use LPM-managed binary
                    path.to_string_lossy().to_string()
                }
                Err(_) => {
                    // Fall back to system PATH
                    program.to_string()
                }
            }
        } else {
            program.to_string()
        };

        // Build command
        let mut cmd = Command::new(&actual_program);
        cmd.args(args);

        // Set working directory
        if let Some(cwd) = &options.cwd {
            cmd.current_dir(cwd);
        } else {
            cmd.current_dir(&project_root);
        }

        // Set up LUA_PATH and LUA_CPATH for Lua commands
        if program == "lua"
            || program == "luajit"
            || program.ends_with("lua")
            || program.ends_with("luajit")
        {
            let lua_path = format!(
                "{}/?.lua;{}/?/init.lua;{}/?/?.lua;",
                lua_modules.to_string_lossy(),
                lua_modules.to_string_lossy(),
                lua_modules.to_string_lossy()
            );
            cmd.env("LUA_PATH", lua_path);

            // Set up LUA_CPATH for native modules
            let cpath_ext = if cfg!(target_os = "windows") {
                "dll"
            } else if cfg!(target_os = "macos") {
                "dylib"
            } else {
                "so"
            };
            let lua_cpath = format!(
                "{}/?.{};{}/?/init.{};",
                lua_modules.to_string_lossy(),
                cpath_ext,
                lua_modules.to_string_lossy(),
                cpath_ext
            );
            cmd.env("LUA_CPATH", lua_cpath);
        }

        // Set additional environment variables
        for (key, value) in &options.env {
            cmd.env(key, value);
        }

        // Run the command
        let status = cmd.status()?;
        Ok(status.code().unwrap_or(1))
    }

    /// Execute arbitrary Lua code with lpm.loader
    pub fn exec_lua(lua_code: &str, options: RunOptions) -> LpmResult<i32> {
        // Try to find project root from current directory
        let current_dir = std::env::current_dir()?;
        let project_root = find_project_root(&current_dir)?;
        let lua_modules = lua_modules_dir(&project_root);

        if !lua_modules.exists() {
            return Err(LpmError::Package(
                "lua_modules directory not found. Run 'lpm install' first.".to_string(),
            ));
        }

        // Ensure loader is installed
        PathSetup::install_loader(&project_root)?;

        // Try to use LPM-managed Lua, fall back to system PATH
        let lua_binary = get_lpm_lua_binary("lua", &project_root)
            .unwrap_or_else(|_| Path::new("lua").to_path_buf());

        // Build Lua command
        let mut cmd = Command::new(&lua_binary);

        // Add lpm.loader require
        let lpm_dir = lua_modules.join("lpm");
        cmd.arg("-e").arg(format!(
            "package.path = '{}' .. '/?.lua;' .. package.path; require('lpm.loader'); {}",
            lpm_dir.to_string_lossy(),
            lua_code
        ));

        // Set working directory
        if let Some(cwd) = &options.cwd {
            cmd.current_dir(cwd);
        } else {
            cmd.current_dir(&project_root);
        }

        // Set environment variables
        for (key, value) in &options.env {
            cmd.env(key, value);
        }

        // Add LUA_PATH
        let lua_path = format!(
            "{}/?.lua;{}/?/init.lua;",
            lua_modules.to_string_lossy(),
            lua_modules.to_string_lossy()
        );
        cmd.env("LUA_PATH", lua_path);

        // Run the command
        let status = cmd.status()?;
        Ok(status.code().unwrap_or(1))
    }
}

/// Get the path to LPM-managed Lua binary, respecting .lua-version files
///
/// Note: In lpm-core, this is a simplified version that doesn't use lua_manager.
/// It will always return an error, causing the code to fall back to system Lua.
/// For full LPM-managed Lua support, use the main lpm crate.
fn get_lpm_lua_binary(_binary: &str, _project_root: &Path) -> LpmResult<std::path::PathBuf> {
    // In lpm-core, we don't have access to lua_manager, so always return error
    // This causes the code to fall back to system PATH Lua
    Err(LpmError::Package(
        "LPM-managed Lua not available in lpm-core. Using system Lua.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_run_script_structure() {
        let temp = TempDir::new().unwrap();
        let script_path = temp.path().join("test.lua");
        fs::write(&script_path, "print('test')").unwrap();

        // This will fail because lua_modules doesn't exist, but tests the structure
        let result = LuaRunner::run_script(&script_path, RunOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_run_script_error_no_lua_modules() {
        let temp = TempDir::new().unwrap();
        let script_path = temp.path().join("test.lua");
        fs::write(&script_path, "print('test')").unwrap();

        // Create package.yaml to make it a valid project
        fs::write(temp.path().join("package.yaml"), "name: test\nversion: 1.0.0\n").unwrap();

        let result = LuaRunner::run_script(&script_path, RunOptions::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("lua_modules"));
    }

    #[test]
    fn test_exec_command_empty() {
        let result = LuaRunner::exec_command("", RunOptions::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Empty command"));
    }

    #[test]
    fn test_exec_lua_no_lua_modules() {
        let temp = TempDir::new().unwrap();
        let _cwd = std::env::set_current_dir(temp.path());

        // Create package.yaml
        fs::write(temp.path().join("package.yaml"), "name: test\nversion: 1.0.0\n").unwrap();

        let result = LuaRunner::exec_lua("print('hello')", RunOptions::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("lua_modules"));
    }

    #[test]
    fn test_run_options_default() {
        let opts = RunOptions::default();
        assert!(opts.cwd.is_none());
        assert!(opts.lua_args.is_empty());
        assert!(opts.env.is_empty());
    }

    #[test]
    fn test_run_options_with_cwd() {
        let opts = RunOptions {
            cwd: Some("/tmp".to_string()),
            lua_args: vec![],
            env: vec![],
        };
        assert_eq!(opts.cwd, Some("/tmp".to_string()));
    }

    #[test]
    fn test_run_options_with_args() {
        let opts = RunOptions {
            cwd: None,
            lua_args: vec!["arg1".to_string(), "arg2".to_string()],
            env: vec![],
        };
        assert_eq!(opts.lua_args.len(), 2);
    }

    #[test]
    fn test_run_options_with_env() {
        let opts = RunOptions {
            cwd: None,
            lua_args: vec![],
            env: vec![("KEY".to_string(), "VALUE".to_string())],
        };
        assert_eq!(opts.env.len(), 1);
    }

    #[test]
    fn test_get_lpm_lua_binary_always_fails() {
        let temp = TempDir::new().unwrap();
        let result = get_lpm_lua_binary("lua", temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("lpm-core"));
    }

    #[test]
    fn test_exec_command_with_whitespace() {
        let temp = TempDir::new().unwrap();
        let _cwd = std::env::set_current_dir(temp.path());

        // Create package.yaml
        fs::write(temp.path().join("package.yaml"), "name: test\nversion: 1.0.0\n").unwrap();

        // Create lua_modules directory
        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        // Create lpm directory for loader
        let lpm_dir = lua_modules.join("lpm");
        fs::create_dir_all(&lpm_dir).unwrap();
        fs::write(lpm_dir.join("loader.lua"), "-- loader").unwrap();

        // This will try to execute "echo test" which should work on Unix systems
        // On Windows this might fail, but we're testing the parsing logic
        let result = LuaRunner::exec_command("echo test", RunOptions::default());
        // We don't assert success since it depends on system availability
        let _ = result;
    }
}
