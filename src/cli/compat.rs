use depot::core::path::find_project_root;
use depot::core::{DepotError, DepotResult};
use depot::lua_analysis::{report, scanner};
use depot::PackageManifest;
use std::env;
use std::path::Path;

pub fn run(quiet: bool, json: bool) -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;
    run_in_dir(&current_dir, quiet, json)
}

pub fn run_in_dir(dir: &Path, quiet: bool, json: bool) -> DepotResult<()> {
    let project_root = find_project_root(dir)?;
    let manifest = PackageManifest::load(&project_root)?;

    if !json {
        println!("Scanning Lua files for version compatibility...");
    }

    let file_results = scanner::scan_project(&project_root)?;
    let report = report::build_report(file_results, Some(&manifest.lua_version));

    if json {
        let json_report = report::to_json_report(&report);
        let output = serde_json::to_string_pretty(&json_report)
            .map_err(|e| DepotError::Package(format!("Failed to serialize report: {}", e)))?;
        println!("{}", output);
    } else {
        let output = report::format_report(&report, quiet);
        print!("{}", output);
    }

    // Return error if configured version is incompatible
    if report.config_compatible == Some(false) {
        return Err(DepotError::Package(format!(
            "Configured lua_version \"{}\" is incompatible with detected code features",
            manifest.lua_version
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_run_no_project() {
        let temp = TempDir::new().unwrap();
        let subdir = temp.path().join("not-a-project");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = run_in_dir(&subdir, false, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_empty_project() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\nlua_version: \"5.4\"\n",
        )
        .unwrap();

        let result = run_in_dir(temp.path(), false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_compatible_code() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\nlua_version: \"5.4\"\n",
        )
        .unwrap();

        let src_dir = temp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("main.lua"),
            "local x = table.move(t, 1, #t, 2)\n",
        )
        .unwrap();

        let result = run_in_dir(temp.path(), false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_incompatible_code() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\nlua_version: \"5.4\"\n",
        )
        .unwrap();

        let src_dir = temp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("main.lua"), "setfenv(1, {})\n").unwrap();

        let result = run_in_dir(temp.path(), false, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_json_output() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\nlua_version: \"5.4\"\n",
        )
        .unwrap();

        let result = run_in_dir(temp.path(), false, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_skips_lua_modules() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\nlua_version: \"5.4\"\n",
        )
        .unwrap();

        // Create lua_modules with incompatible code (should be skipped)
        let modules_dir = temp.path().join("lua_modules").join("old_lib");
        std::fs::create_dir_all(&modules_dir).unwrap();
        std::fs::write(modules_dir.join("init.lua"), "setfenv(1, {})\n").unwrap();

        let result = run_in_dir(temp.path(), false, false);
        assert!(result.is_ok()); // Should pass because lua_modules is skipped
    }
}
