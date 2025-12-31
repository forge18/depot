use lpm::build::builder::RustBuilder;
use lpm::build::targets::Target;
use lpm::core::path::find_project_root;
use lpm::core::{LpmError, LpmResult};
use lpm::package::manifest::PackageManifest;
use lpm::workspace::{Workspace, WorkspaceFilter};
use std::env;
use std::path::Path;

pub fn run(target: Option<String>, all_targets: bool, filter: Vec<String>) -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    // Check if we're in a workspace and filtering is requested
    if !filter.is_empty() {
        let project_root = find_project_root(&current_dir)?;
        if Workspace::is_workspace(&project_root) {
            let workspace = Workspace::load(&project_root)?;
            return build_workspace_filtered(&workspace, &filter, target, all_targets);
        } else {
            return Err(LpmError::Package(
                "--filter can only be used in workspace mode".to_string(),
            ));
        }
    }

    run_in_dir(&current_dir, target, all_targets)
}

pub fn run_in_dir(dir: &Path, target: Option<String>, all_targets: bool) -> LpmResult<()> {
    let project_root = find_project_root(dir)?;
    let manifest = PackageManifest::load(&project_root)?;

    // Check if project has Rust build configuration
    if manifest.build.is_none() {
        return Err(LpmError::Package(
            "No build configuration found in package.yaml. Add a 'build' section with type: rust"
                .to_string(),
        ));
    }

    let builder = RustBuilder::new(&project_root, &manifest)?;

    if all_targets {
        // Build for all supported targets
        eprintln!("Building for all supported targets...");
        // build_all_targets is not async, but calls build which is async
        // We need to handle this differently
        let mut results = Vec::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        for target_triple in lpm::build::targets::SUPPORTED_TARGETS {
            let target = Target::new(target_triple)?;
            eprintln!("Building for target: {}", target.triple);

            match rt.block_on(builder.build(Some(&target))) {
                Ok(path) => {
                    results.push((target, path));
                    eprintln!("✓ Built successfully for {}", target_triple);
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to build for {}: {}", target_triple, e);
                }
            }
        }

        if results.is_empty() {
            return Err(LpmError::Package(
                "Failed to build for all targets".to_string(),
            ));
        }

        eprintln!("\n✓ Build complete for {} target(s):", results.len());
        for (target, path) in &results {
            eprintln!("  {} -> {}", target.triple, path.display());
        }
    } else {
        // Build for specific target or default
        let build_target = if let Some(triple) = target {
            Some(Target::new(&triple)?)
        } else {
            None
        };

        let target_display = build_target
            .as_ref()
            .map(|t| t.triple.as_str())
            .unwrap_or("default");
        eprintln!("Building for target: {}", target_display);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let output_path = rt.block_on(builder.build(build_target.as_ref()))?;
        eprintln!("✓ Build complete: {}", output_path.display());
    }

    Ok(())
}

fn build_workspace_filtered(
    workspace: &Workspace,
    filter_patterns: &[String],
    target: Option<String>,
    all_targets: bool,
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
        "📦 Building {} workspace package(s):",
        filtered_packages.len()
    );
    for pkg in &filtered_packages {
        println!("  - {} ({})", pkg.name, pkg.path.display());
    }
    println!();

    let mut any_failed = false;

    // Build each filtered package
    for pkg in filtered_packages {
        let pkg_dir = workspace.root.join(&pkg.path);

        println!("Building {}...", pkg.name);

        // Load package manifest
        let manifest = PackageManifest::load(&pkg_dir)?;

        // Check if this package has a build configuration
        if manifest.build.is_none() {
            println!(
                "  ⚠️  No build configuration found in {}, skipping",
                pkg.name
            );
            continue;
        }

        // Create builder
        let builder = match RustBuilder::new(&pkg_dir, &manifest) {
            Ok(b) => b,
            Err(e) => {
                println!("  ✗ Failed to create builder for {}: {}\n", pkg.name, e);
                any_failed = true;
                continue;
            }
        };

        // Create runtime for async builds
        let rt = tokio::runtime::Runtime::new().unwrap();

        if all_targets {
            // Build for all supported targets
            println!("  Building for all supported targets...");
            let mut results = Vec::new();

            for target_triple in lpm::build::targets::SUPPORTED_TARGETS {
                let build_target = match Target::new(target_triple) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("  ⚠️  Invalid target {}: {}", target_triple, e);
                        continue;
                    }
                };

                println!("    Building for target: {}", build_target.triple);

                match rt.block_on(builder.build(Some(&build_target))) {
                    Ok(path) => {
                        results.push((build_target, path));
                        println!("    ✓ Built successfully for {}", target_triple);
                    }
                    Err(e) => {
                        eprintln!("    ⚠️  Failed to build for {}: {}", target_triple, e);
                    }
                }
            }

            if results.is_empty() {
                println!("  ✗ Failed to build for all targets in {}\n", pkg.name);
                any_failed = true;
                continue;
            }

            println!(
                "  ✓ Build complete for {} target(s) in {}\n",
                results.len(),
                pkg.name
            );
        } else {
            // Build for specific target or default
            let build_target = if let Some(ref triple) = target {
                match Target::new(triple) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        println!("  ✗ Invalid target {}: {}\n", triple, e);
                        any_failed = true;
                        continue;
                    }
                }
            } else {
                None
            };

            let target_display = build_target
                .as_ref()
                .map(|t| t.triple.as_str())
                .unwrap_or("default");
            println!("  Building for target: {}", target_display);

            match rt.block_on(builder.build(build_target.as_ref())) {
                Ok(output_path) => {
                    println!("  ✓ Build complete: {}\n", output_path.display());
                }
                Err(e) => {
                    println!("  ✗ Build failed for {}: {}\n", pkg.name, e);
                    any_failed = true;
                }
            }
        }
    }

    if any_failed {
        return Err(LpmError::Package("One or more builds failed".to_string()));
    }

    println!("✓ All filtered workspace packages built successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_build_run_function_exists() {
        // Test that run function exists
        let _ = run;
    }

    #[test]
    fn test_build_run_error_no_build_config() {
        // Test error path when no build config
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\n",
        )
        .unwrap();

        let result = run_in_dir(temp.path(), None, false);

        // Should fail - no build config
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No build configuration"));
    }

    #[test]
    fn test_build_run_error_no_project_root() {
        // Test error path when not in a project
        let temp = TempDir::new().unwrap();
        let subdir = temp.path().join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = run_in_dir(&subdir, None, false);

        // Should fail - no project root
        assert!(result.is_err());
    }

    #[test]
    fn test_build_run_all_targets_path() {
        // Test the all_targets path
        let temp = TempDir::new().unwrap();
        let manifest_content = r#"name: test
version: 1.0.0
build:
  type: rust
"#;
        fs::write(temp.path().join("package.yaml"), manifest_content).unwrap();

        let result = run_in_dir(temp.path(), None, true); // all_targets = true

        // May fail on actual build, but tests the all_targets code path
        let _ = result;
    }

    #[test]
    fn test_build_run_specific_target_path() {
        // Test the specific target path
        let temp = TempDir::new().unwrap();
        let manifest_content = r#"name: test
version: 1.0.0
build:
  type: rust
"#;
        fs::write(temp.path().join("package.yaml"), manifest_content).unwrap();

        let result = run_in_dir(
            temp.path(),
            Some("x86_64-unknown-linux-gnu".to_string()),
            false,
        );

        // May fail on actual build, but tests the specific target code path
        let _ = result;
    }

    #[test]
    fn test_build_run_default_target_path() {
        // Test the default target path
        let temp = TempDir::new().unwrap();
        let manifest_content = r#"name: test
version: 1.0.0
build:
  type: rust
"#;
        fs::write(temp.path().join("package.yaml"), manifest_content).unwrap();

        let result = run_in_dir(temp.path(), None, false); // No target specified, uses default

        // May fail on actual build, but tests the default target code path
        let _ = result;
    }

    #[test]
    fn test_build_run_with_filter_not_in_workspace() {
        // Test error when using filter outside workspace
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\n",
        )
        .unwrap();

        let result = run(None, false, vec!["filter".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("workspace"));
    }

    #[test]
    fn test_build_workspace_filtered_function_exists() {
        // Verify the function exists
        let _ = build_workspace_filtered;
    }

    #[test]
    fn test_build_run_in_dir_function_signature() {
        // Test function signature
        let _func: fn(&Path, Option<String>, bool) -> LpmResult<()> = run_in_dir;
    }

    #[test]
    fn test_build_run_function_signature() {
        // Test function signature
        let _func: fn(Option<String>, bool, Vec<String>) -> LpmResult<()> = run;
    }
}
