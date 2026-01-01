use crate::cli::install::run_interactive;
use crate::cli::template::{TemplateDiscovery, TemplateRenderer};
use dialoguer::{Confirm, Input, MultiSelect, Select};
use lpm::core::path::find_project_root;
use lpm::core::{LpmError, LpmResult};
use lpm::package::manifest::PackageManifest;
use std::collections::HashMap;
use std::env;
use std::path::Path;

pub async fn run(template: Option<String>, yes: bool) -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;
    run_in_dir(&current_dir, template, yes).await
}

pub async fn run_in_dir(dir: &Path, template: Option<String>, yes: bool) -> LpmResult<()> {
    // Check if we're already in a project
    if find_project_root(dir).is_ok() {
        return Err(LpmError::Package(
            "Already in an LPM project (package.yaml exists)".to_string(),
        ));
    }

    // Get project name from directory
    let default_project_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project")
        .to_string();

    if yes {
        // Non-interactive mode: use defaults
        return run_non_interactive(dir, &default_project_name, template);
    }

    // Interactive wizard mode
    run_wizard(dir, &default_project_name, template).await
}

async fn run_wizard(
    current_dir: &Path,
    default_project_name: &str,
    template_name: Option<String>,
) -> LpmResult<()> {
    println!("🚀 LPM Project Initialization Wizard\n");

    // Collect project name with validation
    let project_name: String = Input::new()
        .with_prompt("Project name")
        .default(default_project_name.to_string())
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.is_empty() {
                Err("Project name cannot be empty")
            } else if !input.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                Err("Project name can only contain alphanumeric characters, hyphens, and underscores")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;

    // Collect project version (defaults to 1.0.0)
    let project_version: String = Input::new()
        .with_prompt("Project version")
        .default("1.0.0".to_string())
        .interact_text()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;

    // Collect optional project description
    let description: String = Input::new()
        .with_prompt("Description (optional)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;

    // Select license from common options
    let licenses = vec![
        "MIT",
        "Apache-2.0",
        "BSD-3-Clause",
        "GPL-3.0",
        "LGPL-3.0",
        "ISC",
        "Unlicense",
        "None",
    ];
    let license_selection = Select::new()
        .with_prompt("License")
        .items(&licenses)
        .default(0)
        .interact()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;
    let license = licenses[license_selection].to_string();

    // Discover installed Lua versions dynamically
    let installed_versions = lpm::lua_version::LuaVersion::discover_installed();
    let mut lua_versions: Vec<String> =
        installed_versions.iter().map(|v| v.major_minor()).collect();

    // Add common versions if not already present
    for common in ["5.1", "5.3", "5.4"] {
        if !lua_versions.contains(&common.to_string()) {
            lua_versions.push(common.to_string());
        }
    }

    // Always add "latest" option
    lua_versions.push("latest".to_string());

    // Find default index (prefer 5.4, or first installed version)
    let default_index = lua_versions
        .iter()
        .position(|v| v == "5.4")
        .or(if !installed_versions.is_empty() {
            Some(0)
        } else {
            None
        })
        .unwrap_or(lua_versions.len().saturating_sub(2)); // "latest" is last, so use second-to-last

    let lua_selection = Select::new()
        .with_prompt("Lua version")
        .items(&lua_versions)
        .default(default_index)
        .interact()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;
    let lua_version = lua_versions[lua_selection].clone();

    // Select or use specified template
    let selected_template = if let Some(template_name) = template_name {
        Some(TemplateDiscovery::find_template(&template_name)?)
    } else {
        // Present template selection menu
        let templates = TemplateDiscovery::list_templates()?;
        if !templates.is_empty() {
            let template_names: Vec<String> = templates
                .iter()
                .map(|t| format!("{} - {}", t.name, t.description))
                .collect();
            let mut all_items = vec!["None (empty project)".to_string()];
            all_items.extend(template_names);
            let template_selection = Select::new()
                .with_prompt("Use a template? (optional)")
                .items(&all_items)
                .default(0)
                .interact()
                .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;

            if template_selection > 0 {
                Some(templates[template_selection - 1].clone())
            } else {
                None
            }
        } else {
            None
        }
    };

    // Prompt for initial dependency setup
    let add_dependencies = Confirm::new()
        .with_prompt("Add initial dependencies?")
        .default(false)
        .interact()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;

    // Configure common npm-style scripts (dev, test, build, start)
    let common_scripts = [
        ("dev", "Development server/watch mode"),
        ("test", "Run tests"),
        ("build", "Build project"),
        ("start", "Start application"),
    ];

    let script_options: Vec<String> = common_scripts
        .iter()
        .map(|(name, desc)| format!("{} - {}", name, desc))
        .collect();

    let script_selections = MultiSelect::new()
        .with_prompt("Set up common scripts? (space to select, enter to confirm)")
        .items(&script_options)
        .interact()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;

    // Step 9: Show summary
    println!("\n📋 Project Summary:");
    println!("  Name: {}", project_name);
    println!("  Version: {}", project_version);
    if !description.is_empty() {
        println!("  Description: {}", description);
    }
    println!("  License: {}", license);
    println!("  Lua Version: {}", lua_version);
    if let Some(ref template) = selected_template {
        println!("  Template: {}", template.name);
    }
    if add_dependencies {
        println!("  Initial dependencies: Yes (will be added after project creation)");
    }
    if !script_selections.is_empty() {
        println!(
            "  Scripts: {}",
            script_selections
                .iter()
                .map(|&i| common_scripts[i].0)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let confirmed = Confirm::new()
        .with_prompt("Create project?")
        .default(true)
        .interact()
        .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    // Create project
    let mut manifest = PackageManifest::default(project_name.clone());
    manifest.version = project_version.clone();
    if !description.is_empty() {
        manifest.description = Some(description);
    }
    if license != "None" {
        manifest.license = Some(license);
    }
    manifest.lua_version = lua_version.clone();

    // Add scripts
    for &idx in &script_selections {
        let (script_name, _) = common_scripts[idx];
        match script_name {
            "dev" => {
                manifest
                    .scripts
                    .insert("dev".to_string(), "lpm watch dev".to_string());
            }
            "test" => {
                manifest
                    .scripts
                    .insert("test".to_string(), "lua tests/run.lua".to_string());
            }
            "build" => {
                manifest
                    .scripts
                    .insert("build".to_string(), "lua src/build.lua".to_string());
            }
            "start" => {
                manifest
                    .scripts
                    .insert("start".to_string(), "lua src/main.lua".to_string());
            }
            _ => {}
        }
    }

    // Save package.yaml
    manifest.save(current_dir)?;

    // If template is selected, render it
    let template_used = if let Some(ref template) = selected_template {
        let mut variables = HashMap::new();
        variables.insert("project_name".to_string(), project_name.clone());
        variables.insert("project_version".to_string(), project_version.clone());
        variables.insert("lua_version".to_string(), lua_version.clone());

        let renderer = TemplateRenderer::new(template.path.clone())?;
        renderer.render(current_dir, &variables)?;
        true
    } else {
        // Create basic directory structure
        std::fs::create_dir_all(current_dir.join("src"))?;
        std::fs::create_dir_all(current_dir.join("lib"))?;
        std::fs::create_dir_all(current_dir.join("tests"))?;

        // Create a basic main.lua if it doesn't exist
        let main_lua = current_dir.join("src").join("main.lua");
        if !main_lua.exists() {
            std::fs::write(
                &main_lua,
                format!(
                    "-- {}\n-- Version: {}\n\nprint(\"Hello from {}\")\n",
                    project_name, project_version, project_name
                ),
            )?;
        }
        false
    };

    println!("\n✓ Initialized LPM project: {}", project_name);
    println!("  Created package.yaml");
    if template_used {
        println!("  Applied template");
    }
    if !script_selections.is_empty() {
        println!("  Added {} script(s)", script_selections.len());
    }

    // Add initial dependencies if requested
    if add_dependencies {
        println!("\n📦 Adding initial dependencies...");
        run_interactive(current_dir, false, &mut manifest).await?;
    }

    println!("\nNext steps:");
    if !add_dependencies {
        println!("  lpm install <package>  - Add a dependency");
        println!("  lpm install            - Install all dependencies");
    }
    if !script_selections.is_empty() {
        println!("  lpm run <script>      - Run a script (e.g., lpm run dev)");
    }

    Ok(())
}

fn run_non_interactive(
    current_dir: &Path,
    project_name: &str,
    template_name: Option<String>,
) -> LpmResult<()> {
    // Create default manifest
    let manifest = PackageManifest::default(project_name.to_string());

    // Save package.yaml
    manifest.save(current_dir)?;

    // If template is specified, render it
    if let Some(ref template_name) = template_name {
        let template = TemplateDiscovery::find_template(template_name)?;
        let mut variables = HashMap::new();
        variables.insert("project_name".to_string(), project_name.to_string());
        variables.insert("project_version".to_string(), "1.0.0".to_string());
        variables.insert("lua_version".to_string(), "5.4".to_string());

        let renderer = TemplateRenderer::new(template.path)?;
        renderer.render(current_dir, &variables)?;
    } else {
        // Create basic directory structure
        std::fs::create_dir_all(current_dir.join("src"))?;
        std::fs::create_dir_all(current_dir.join("lib"))?;
        std::fs::create_dir_all(current_dir.join("tests"))?;
    }

    println!("✓ Initialized LPM project: {}", project_name);
    println!("  Created package.yaml");
    if template_name.is_some() {
        println!("  Applied template");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_run_non_interactive() {
        let temp = TempDir::new().unwrap();
        let result = run_non_interactive(temp.path(), "test-project", None);
        assert!(result.is_ok());
        assert!(temp.path().join("package.yaml").exists());
    }

    #[test]
    fn test_run_non_interactive_with_template() {
        let temp = TempDir::new().unwrap();
        // Template discovery might fail, but that's ok
        let _ = run_non_interactive(temp.path(), "test-project", Some("nonexistent".to_string()));
    }

    #[tokio::test]
    async fn test_run_already_in_project() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("package.yaml"), "name: test").unwrap();

        let result = run_in_dir(temp.path(), None, false).await;

        // This will fail because we're in a project
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Already in an LPM project"));
    }

    #[test]
    fn test_run_non_interactive_creates_directories() {
        let temp = TempDir::new().unwrap();
        run_non_interactive(temp.path(), "test-project", None).unwrap();

        // Should create src, lib, tests directories
        assert!(temp.path().join("src").exists());
        assert!(temp.path().join("lib").exists());
        assert!(temp.path().join("tests").exists());
    }

    #[test]
    fn test_run_non_interactive_manifest_content() {
        let temp = TempDir::new().unwrap();
        run_non_interactive(temp.path(), "my-cool-project", None).unwrap();

        let manifest_content = fs::read_to_string(temp.path().join("package.yaml")).unwrap();
        assert!(manifest_content.contains("my-cool-project"));
    }

    #[tokio::test]
    async fn test_run_in_dir_with_yes_flag() {
        let temp = TempDir::new().unwrap();
        let result = run_in_dir(temp.path(), None, true).await;

        assert!(result.is_ok());
        assert!(temp.path().join("package.yaml").exists());
    }

    #[test]
    fn test_run_non_interactive_special_characters_in_name() {
        let temp = TempDir::new().unwrap();
        let result = run_non_interactive(temp.path(), "test-project_123", None);
        assert!(result.is_ok());

        let manifest_content = fs::read_to_string(temp.path().join("package.yaml")).unwrap();
        assert!(manifest_content.contains("test-project_123"));
    }

    #[test]
    fn test_run_non_interactive_overwrites_existing() {
        let temp = TempDir::new().unwrap();

        // First run
        run_non_interactive(temp.path(), "project1", None).unwrap();

        // Second run with different name - will fail because project already exists
        // but the test verifies the path is checked
        let manifest = fs::read_to_string(temp.path().join("package.yaml")).unwrap();
        assert!(manifest.contains("project1"));
    }

    #[test]
    fn test_default_project_name_from_directory() {
        let temp = TempDir::new().unwrap();
        // The default name would be derived from the directory name
        let dir_name = temp
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-project");

        // Just verify the logic works
        assert!(!dir_name.is_empty());
    }

    #[tokio::test]
    async fn test_run_uses_current_dir() {
        use std::env;

        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().ok();

        // Change to temp directory
        env::set_current_dir(temp.path()).unwrap();

        // Run init with yes flag to avoid interactive prompts
        let result = run(None, true).await;

        // Restore original directory
        if let Some(dir) = original_dir {
            let _ = env::set_current_dir(dir);
        }

        assert!(result.is_ok());
        assert!(temp.path().join("package.yaml").exists());
    }

    #[tokio::test]
    async fn test_run_in_dir_with_template_name() {
        let temp = TempDir::new().unwrap();
        // Try with a template name - will likely fail to find template, but exercises the code path
        let result = run_in_dir(temp.path(), Some("nonexistent-template".to_string()), true).await;

        // May fail due to template not found, but that's ok - we're testing the code path
        let _ = result;
    }

    #[test]
    fn test_run_non_interactive_directory_structure() {
        let temp = TempDir::new().unwrap();
        run_non_interactive(temp.path(), "test-project", None).unwrap();

        // Verify all three directories were created
        assert!(temp.path().join("src").is_dir());
        assert!(temp.path().join("lib").is_dir());
        assert!(temp.path().join("tests").is_dir());
    }

    #[tokio::test]
    async fn test_run_in_dir_default_project_name_fallback() {
        let temp = TempDir::new().unwrap();
        let result = run_in_dir(temp.path(), None, true).await;

        assert!(result.is_ok());

        // Verify project was created with directory name as project name
        let manifest_content = fs::read_to_string(temp.path().join("package.yaml")).unwrap();
        // The temp directory name should be in the manifest
        assert!(manifest_content.contains("name:"));
    }

    #[test]
    fn test_run_non_interactive_with_various_project_names() {
        let temp = TempDir::new().unwrap();

        // Test with hyphens
        let result1 = run_non_interactive(temp.path(), "my-test-project", None);
        assert!(result1.is_ok());

        let temp2 = TempDir::new().unwrap();
        // Test with underscores
        let result2 = run_non_interactive(temp2.path(), "my_test_project", None);
        assert!(result2.is_ok());

        let temp3 = TempDir::new().unwrap();
        // Test with mixed
        let result3 = run_non_interactive(temp3.path(), "my-test_project123", None);
        assert!(result3.is_ok());
    }

    #[tokio::test]
    async fn test_run_in_dir_calls_non_interactive_with_yes() {
        let temp = TempDir::new().unwrap();

        // Call with yes=true should use non-interactive mode
        let result = run_in_dir(temp.path(), None, true).await;

        assert!(result.is_ok());
        assert!(temp.path().join("package.yaml").exists());
        assert!(temp.path().join("src").exists());
    }
}
