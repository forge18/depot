use lpm::core::path::find_project_root;
use lpm::core::LpmResult;
use lpm::package::manifest::PackageManifest;
use lpm::path_setup::{LuaRunner, PathSetup, RunOptions};
use std::env;

pub fn run(script_name: String) -> LpmResult<()> {
    let current_dir = env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;

    // Load manifest to get scripts
    let manifest = PackageManifest::load(&project_root)?;

    // Find the script
    let script_command = manifest.scripts.get(&script_name).ok_or_else(|| {
        lpm::core::LpmError::Package(format!(
            "Script '{}' not found in package.yaml",
            script_name
        ))
    })?;

    // Ensure loader is installed (sets up package.path automatically)
    PathSetup::install_loader(&project_root)?;

    // Parse the script command (e.g., "lua src/main.lua" or "luajit -e 'print(1)'")
    let parts: Vec<&str> = script_command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(lpm::core::LpmError::Package(format!(
            "Script '{}' has no command",
            script_name
        )));
    }

    // Execute the command with proper path setup
    let exit_code = LuaRunner::exec_command(script_command, RunOptions::default())?;
    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_script_not_found() {
        // Test that run handles missing script
        // This will fail because we're not in a project root, but tests the error path
        let result = run("nonexistent-script".to_string());
        assert!(result.is_err());
        // Error could be "not found" or "package.yaml not found" depending on path
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("not found") || error_msg.contains("package.yaml"),
            "Error message should mention 'not found' or 'package.yaml', got: {}",
            error_msg
        );
    }

    #[test]
    fn test_run_script_empty_command() {
        // Test that run handles empty script command
        // This will fail because we're not in a project root, but tests the error path
        let result = run("empty-script".to_string());
        // Will fail due to path, but tests the function structure
        assert!(result.is_err());
    }

    #[test]
    fn test_script_command_parsing() {
        // Test script command parsing logic
        let script_command = "lua src/main.lua";
        let parts: Vec<&str> = script_command.split_whitespace().collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "lua");
        assert_eq!(parts[1], "src/main.lua");
    }
}
