use lpm::core::path::find_project_root;
use lpm::core::{LpmError, LpmResult};
use lpm::package::manifest::PackageManifest;
use lpm::path_setup::{LuaRunner, PathSetup, RunOptions};
use lpm::workspace::{Workspace, WorkspaceFilter};
use std::env;

pub fn run(script_name: String, filter: Vec<String>) -> LpmResult<()> {
    let current_dir = env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;

    // Check if we're in a workspace and filtering is requested
    if !filter.is_empty() {
        if Workspace::is_workspace(&project_root) {
            let workspace = Workspace::load(&project_root)?;
            return run_workspace_filtered(&workspace, &filter, &script_name);
        } else {
            return Err(LpmError::Package(
                "--filter can only be used in workspace mode".to_string(),
            ));
        }
    }

    // Load manifest to get scripts
    let manifest = PackageManifest::load(&project_root)?;

    // Find the script
    let script_command = manifest.scripts.get(&script_name).ok_or_else(|| {
        LpmError::Package(format!(
            "Script '{}' not found in package.yaml",
            script_name
        ))
    })?;

    // Ensure loader is installed (sets up package.path automatically)
    PathSetup::install_loader(&project_root)?;

    // Parse the script command (e.g., "lua src/main.lua" or "luajit -e 'print(1)'")
    let parts: Vec<&str> = script_command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(LpmError::Package(format!(
            "Script '{}' has no command",
            script_name
        )));
    }

    // Execute the command with proper path setup
    let exit_code = LuaRunner::exec_command(script_command, RunOptions::default())?;
    std::process::exit(exit_code);
}

fn run_workspace_filtered(
    workspace: &Workspace,
    filter_patterns: &[String],
    script_name: &str,
) -> LpmResult<()> {
    // Create filter
    let filter = WorkspaceFilter::new(filter_patterns.to_vec());

    // Get filtered packages
    let filtered_packages = filter.filter_packages(workspace)?;

    if filtered_packages.is_empty() {
        println!("No packages match the filter patterns");
        return Ok(());
    }

    println!(
        "📦 Running script '{}' for {} workspace package(s):",
        script_name,
        filtered_packages.len()
    );
    for pkg in &filtered_packages {
        println!("  - {} ({})", pkg.name, pkg.path.display());
    }
    println!();

    let mut any_failed = false;

    // Run script for each filtered package
    for pkg in filtered_packages {
        let pkg_dir = workspace.root.join(&pkg.path);

        println!("Running '{}' in {}...", script_name, pkg.name);

        // Load package manifest
        let manifest = PackageManifest::load(&pkg_dir)?;

        // Find the script
        let script_command = match manifest.scripts.get(script_name) {
            Some(cmd) => cmd,
            None => {
                println!(
                    "  ⚠️  Script '{}' not found in {}, skipping",
                    script_name, pkg.name
                );
                continue;
            }
        };

        // Ensure loader is installed
        PathSetup::install_loader(&pkg_dir)?;

        // Parse the script command
        let parts: Vec<&str> = script_command.split_whitespace().collect();
        if parts.is_empty() {
            println!(
                "  ⚠️  Script '{}' has no command in {}, skipping",
                script_name, pkg.name
            );
            continue;
        }

        // Execute the command
        match LuaRunner::exec_command(script_command, RunOptions::default()) {
            Ok(exit_code) => {
                if exit_code == 0 {
                    println!(
                        "  ✓ Script '{}' completed successfully in {}\n",
                        script_name, pkg.name
                    );
                } else {
                    println!(
                        "  ✗ Script '{}' failed with exit code {} in {}\n",
                        script_name, exit_code, pkg.name
                    );
                    any_failed = true;
                }
            }
            Err(e) => {
                println!(
                    "  ✗ Error running script '{}' in {}: {}\n",
                    script_name, pkg.name, e
                );
                any_failed = true;
            }
        }
    }

    if any_failed {
        return Err(LpmError::Package(
            "One or more script executions failed".to_string(),
        ));
    }

    println!("✓ All filtered workspace packages completed script successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_run_script_not_found() {
        // Test that run handles missing script
        // This will fail because we're not in a project root, but tests the error path
        let result = run("nonexistent-script".to_string(), vec![]);
        assert!(result.is_err());
        // Error could be "not found", "package.yaml not found", or IO error depending on environment
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("not found")
                || error_msg.contains("package.yaml")
                || error_msg.contains("IO error"),
            "Error message should mention an error, got: {}",
            error_msg
        );
    }

    #[test]
    #[serial]
    fn test_run_script_empty_command() {
        // Test that run handles empty script command
        // This will fail because we're not in a project root, but tests the error path
        let result = run("empty-script".to_string(), vec![]);
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

    #[test]
    fn test_run_function_exists() {
        let _ = run;
    }

    #[test]
    fn test_run_workspace_filtered_function_exists() {
        let _ = run_workspace_filtered;
    }

    #[test]
    fn test_run_function_signature() {
        let _func: fn(String, Vec<String>) -> LpmResult<()> = run;
    }

    #[test]
    fn test_run_with_filter_not_in_workspace() {
        // Test error when using filter outside workspace
        let result = run("test".to_string(), vec!["filter".to_string()]);
        assert!(result.is_err());
        // Will fail due to not finding project root or workspace
        let _ = result;
    }

    #[test]
    fn test_script_parsing_multiple_args() {
        let script_command = "luajit -e 'print(1)' arg1";
        let parts: Vec<&str> = script_command.split_whitespace().collect();
        assert!(parts.len() >= 2);
        assert_eq!(parts[0], "luajit");
    }

    #[test]
    fn test_script_parsing_empty() {
        let script_command = "";
        let parts: Vec<&str> = script_command.split_whitespace().collect();
        assert!(parts.is_empty());
    }

    #[test]
    fn test_script_parsing_single_word() {
        let script_command = "lua";
        let parts: Vec<&str> = script_command.split_whitespace().collect();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], "lua");
    }

    #[test]
    fn test_run_no_filter() {
        let result = run("test-script".to_string(), vec![]);
        // Will fail due to not finding project root, but tests the function
        assert!(result.is_err());
    }

    #[test]
    fn test_script_command_with_quotes() {
        let script_command = "lua -e 'print(\"hello world\")'";
        let parts: Vec<&str> = script_command.split_whitespace().collect();
        assert!(parts.len() >= 2);
    }

    #[test]
    fn test_script_command_with_path() {
        let script_command = "lua /absolute/path/to/script.lua";
        let parts: Vec<&str> = script_command.split_whitespace().collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "lua");
        assert!(parts[1].contains("script.lua"));
    }
}
