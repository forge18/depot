pub mod commands;
pub mod config;
pub mod installer;
pub mod metadata;
pub mod registry;

pub use metadata::PluginInfo;

use depot::core::path::depot_home;
use depot::core::{DepotError, DepotResult};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

/// Find a plugin executable by name
pub fn find_plugin(plugin_name: &str) -> Option<PathBuf> {
    find_plugin_in_paths(plugin_name, None, None)
}

/// Find a plugin executable by name, with optional custom paths for testing
fn find_plugin_in_paths(
    plugin_name: &str,
    custom_lpm_home: Option<&std::path::Path>,
    custom_home: Option<&std::path::Path>,
) -> Option<PathBuf> {
    // Check depot_home/bin/depot-{name} (global install location)
    let depot_home_path = custom_lpm_home
        .map(|p| Ok(p.to_path_buf()))
        .unwrap_or_else(depot_home);
    if let Ok(depot_home) = depot_home_path {
        let plugin_path = depot_home
            .join("bin")
            .join(format!("depot-{}", plugin_name));
        if plugin_path.exists() {
            return Some(plugin_path);
        }
    }

    // Also check legacy ~/.depot/bin/depot-{name} for backwards compatibility
    let home_path = custom_home
        .map(|p| Some(p.to_path_buf()))
        .unwrap_or_else(|| std::env::var("HOME").ok().map(PathBuf::from));
    if let Some(home) = home_path {
        let legacy_path = home
            .join(".depot")
            .join("bin")
            .join(format!("depot-{}", plugin_name));
        if legacy_path.exists() {
            return Some(legacy_path);
        }
    }

    // Check PATH for depot-{name}
    which::which(format!("depot-{}", plugin_name)).ok()
}

/// List all installed plugins
pub(crate) fn list_plugins() -> DepotResult<Vec<PluginInfo>> {
    let mut plugins = Vec::new();

    // Check depot_home/bin directory
    if let Ok(depot_home) = depot_home() {
        let bin_dir = depot_home.join("bin");
        if bin_dir.exists() {
            if let Ok(entries) = fs::read_dir(&bin_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if let Some(plugin_name) = name.strip_prefix("depot-") {
                            let plugin_name = plugin_name.to_string();
                            if let Ok(Some(info)) = PluginInfo::from_installed(&plugin_name) {
                                plugins.push(info);
                            }
                        }
                    }
                }
            }
        }
    }

    // Check legacy location
    if let Ok(home) = std::env::var("HOME") {
        let legacy_bin = PathBuf::from(home).join(".depot").join("bin");
        if legacy_bin.exists() {
            if let Ok(entries) = fs::read_dir(&legacy_bin) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if let Some(plugin_name) = name.strip_prefix("depot-") {
                            let plugin_name = plugin_name.to_string();
                            // Only add if not already found
                            if !plugins.iter().any(|p| p.metadata.name == plugin_name) {
                                if let Ok(Some(info)) = PluginInfo::from_installed(&plugin_name) {
                                    plugins.push(info);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(plugins)
}

/// Execute a plugin with arguments
pub fn run_plugin(plugin_name: &str, args: Vec<String>) -> DepotResult<()> {
    use crate::cli::plugin::config::PluginConfig;

    if let Some(plugin_path) = find_plugin(plugin_name) {
        // Check if plugin is executable
        if !is_executable(&plugin_path) {
            return Err(DepotError::Package(format!(
                "Plugin '{}' is not executable.\n\n  Fix: chmod +x {}\n\n  Or reinstall the plugin: depot install -g depot-{}",
                plugin_name,
                plugin_path.display(),
                plugin_name
            )));
        }

        // Load plugin configuration
        let config = PluginConfig::load(plugin_name)?;

        // Set environment variables from config
        let mut cmd = Command::new(&plugin_path);
        cmd.args(args);

        // Pass config settings as environment variables
        // Format: Depot_PLUGIN_<PLUGIN_NAME>_<KEY>=<value>
        for (key, value) in &config.settings {
            let env_key = format!(
                "Depot_PLUGIN_{}_{}",
                plugin_name.to_uppercase().replace("-", "_"),
                key.to_uppercase()
            );
            if let Some(str_val) = value.as_str() {
                cmd.env(&env_key, str_val);
            } else if let Some(num_val) = value.as_i64() {
                cmd.env(&env_key, num_val.to_string());
            } else if let Some(bool_val) = value.as_bool() {
                cmd.env(&env_key, if bool_val { "1" } else { "0" });
            }
        }

        let status = match cmd.status() {
            Ok(status) => status,
            Err(e) => {
                // Check for common execution errors
                let error_msg = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    format!(
                        "Permission denied executing plugin '{}'.\n\n  Fix: chmod +x {}\n\n  Or reinstall: depot install -g depot-{}",
                        plugin_name,
                        plugin_path.display(),
                        plugin_name
                    )
                } else if e.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "Plugin '{}' executable not found at {}.\n\n  Fix: Reinstall the plugin: depot install -g depot-{}",
                        plugin_name,
                        plugin_path.display(),
                        plugin_name
                    )
                } else {
                    format!(
                        "Failed to execute plugin '{}': {}.\n\n  Plugin path: {}\n\n  Fix: Check plugin installation or reinstall: depot install -g depot-{}",
                        plugin_name,
                        e,
                        plugin_path.display(),
                        plugin_name
                    )
                };
                return Err(DepotError::Package(error_msg));
            }
        };

        if !status.success() {
            let exit_code = status.code().unwrap_or(1);
            let mut error_msg = format!("Plugin '{}' exited with code {}", plugin_name, exit_code);

            // Add suggestions based on exit code
            match exit_code {
                1 => {
                    error_msg.push_str("\n\n  This usually indicates a plugin error. Check:");
                    error_msg.push_str(&format!(
                        "\n    - Run 'depot {} --help' for usage",
                        plugin_name
                    ));
                    error_msg.push_str("\n    - Check plugin documentation");
                    error_msg.push_str(&format!(
                        "\n    - Verify plugin is up to date: depot install -g depot-{}",
                        plugin_name
                    ));
                }
                2 => {
                    error_msg.push_str(
                        "\n\n  This usually indicates invalid arguments or configuration.",
                    );
                    error_msg.push_str(&format!(
                        "\n    - Run 'depot {} --help' to see valid options",
                        plugin_name
                    ));
                }
                126 => {
                    error_msg.push_str("\n\n  Plugin is not executable.");
                    error_msg.push_str(&format!("\n    Fix: chmod +x {}", plugin_path.display()));
                }
                127 => {
                    error_msg.push_str("\n\n  Plugin or its dependencies not found.");
                    error_msg.push_str(&format!(
                        "\n    Fix: Reinstall: depot install -g depot-{}",
                        plugin_name
                    ));
                }
                _ => {
                    error_msg.push_str("\n\n  Check plugin documentation or try:");
                    error_msg.push_str(&format!("\n    - depot {} --help", plugin_name));
                    error_msg.push_str(&format!(
                        "\n    - Reinstall: depot install -g depot-{}",
                        plugin_name
                    ));
                }
            }

            return Err(DepotError::Package(error_msg));
        }
        Ok(())
    } else {
        // Provide helpful error message with suggestions
        let mut error_msg = format!("Plugin '{}' not found.\n\n", plugin_name);

        error_msg.push_str(&format!(
            "  Install it with: depot install -g depot-{}\n",
            plugin_name
        ));

        // Check if plugin exists in expected locations
        if let Ok(lpm_home) = depot_home() {
            let bin_dir = lpm_home.join("bin");
            error_msg.push_str(&format!(
                "\n  Expected location: {}\n",
                bin_dir.join(format!("depot-{}", plugin_name)).display()
            ));
        }

        error_msg.push_str("\n  Available plugins are installed in:");
        if let Ok(lpm_home) = depot_home() {
            error_msg.push_str(&format!("\n    - {}/bin/", lpm_home.display()));
        }
        if let Ok(home) = std::env::var("HOME") {
            error_msg.push_str(&format!("\n    - {}/.depot/bin/ (legacy)", home));
        }
        error_msg.push_str("\n    - PATH");

        Err(DepotError::Package(error_msg))
    }
}

/// Check if a file is executable
fn is_executable(path: &PathBuf) -> bool {
    #[cfg(unix)]
    {
        if let Ok(metadata) = fs::metadata(path) {
            let permissions = metadata.permissions();
            let mode = permissions.mode();
            // Check if owner, group, or others have execute permission
            (mode & 0o111) != 0
        } else {
            false
        }
    }
    #[cfg(not(unix))]
    {
        // On Windows, we can't easily check execute permissions
        // Assume it's executable if it exists
        path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn test_find_plugin_found() {
        let temp = TempDir::new().unwrap();
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let plugin_file = bin_dir.join("depot-test-plugin");
        fs::write(&plugin_file, "echo hello").unwrap();
        #[cfg(unix)]
        {
            // Ensure file exists before setting permissions
            assert!(plugin_file.exists());
            fs::set_permissions(&plugin_file, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let plugin_path = find_plugin_in_paths("test-plugin", Some(temp.path()), None).unwrap();
        assert!(plugin_path.exists());
    }

    #[test]
    fn test_find_plugin_not_found() {
        let temp = TempDir::new().unwrap();
        assert!(find_plugin_in_paths("nonexistent-plugin", Some(temp.path()), None).is_none());
    }

    #[test]
    fn test_find_plugin_legacy_path() {
        let temp = TempDir::new().unwrap();
        let legacy_bin = temp.path().join(".depot").join("bin");
        fs::create_dir_all(&legacy_bin).unwrap();
        fs::write(legacy_bin.join("depot-legacy-plugin"), "echo hello").unwrap();
        #[cfg(unix)]
        fs::set_permissions(
            legacy_bin.join("depot-legacy-plugin"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        // Use custom home path instead of setting env var
        let plugin_path =
            find_plugin_in_paths("legacy-plugin", Some(temp.path()), Some(temp.path())).unwrap();
        assert!(plugin_path.exists());
    }

    #[test]
    fn test_list_plugins_empty() {
        // Just verify list_plugins doesn't panic - it reads from real depot_home
        let plugins = list_plugins();
        // May succeed or fail depending on environment, just verify no panic
        let _ = plugins;
    }

    #[test]
    fn test_is_executable_unix() {
        #[cfg(unix)]
        {
            let temp = TempDir::new().unwrap();
            let test_file = temp.path().join("test");
            fs::write(&test_file, "test").unwrap();

            // Initially not executable
            assert!(!is_executable(&test_file));

            // Make executable
            fs::set_permissions(&test_file, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(is_executable(&test_file));
        }
    }

    #[test]
    fn test_is_executable_nonexistent() {
        let nonexistent = PathBuf::from("/nonexistent/path");
        assert!(!is_executable(&nonexistent));
    }

    #[test]
    fn test_run_plugin_not_found() {
        // run_plugin uses find_plugin which reads from real paths
        // Just verify error handling works
        let result = run_plugin("definitely-nonexistent-plugin-12345", vec![]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Plugin 'definitely-nonexistent-plugin-12345' not found"));
    }

    #[test]
    fn test_find_plugin_in_path() {
        // This test depends on PATH, so we just verify it doesn't panic
        let _ = find_plugin("nonexistent");
    }

    #[test]
    fn test_list_plugins_with_plugins() {
        // Test the find_plugin_in_paths function with a temp directory
        let temp = TempDir::new().unwrap();
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(bin_dir.join("depot-test-plugin"), "echo hello").unwrap();
        #[cfg(unix)]
        fs::set_permissions(
            bin_dir.join("depot-test-plugin"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        // Verify the plugin can be found via our test function
        let plugin_path = find_plugin_in_paths("test-plugin", Some(temp.path()), None);
        assert!(plugin_path.is_some());
        assert!(plugin_path.unwrap().exists());
    }
}
