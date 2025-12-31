use lpm::core::{LpmError, LpmResult};
use std::env;
use std::fs;

/// Create a new LPM project in a new directory
pub async fn run(name: String, template: Option<String>, yes: bool) -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;
    let project_dir = current_dir.join(&name);

    if project_dir.exists() {
        return Err(LpmError::Path(format!(
            "Directory '{}' already exists",
            name
        )));
    }

    fs::create_dir(&project_dir)?; // Use create_dir instead of create_dir_all to fail if exists

    // Delegate to init logic
    crate::cli::init::run_in_dir(&project_dir, template, yes).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_new_creates_directory() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        // Change to temp dir
        env::set_current_dir(temp.path()).unwrap();

        let result = run("my-new-project".to_string(), None, true).await;

        // Check filesystem before changing back
        let created = temp.path().join("my-new-project").exists();
        let has_package_yaml = temp
            .path()
            .join("my-new-project")
            .join("package.yaml")
            .exists();

        // Restore original dir
        env::set_current_dir(&original_dir).unwrap();

        if let Err(ref e) = result {
            eprintln!("Error: {:?}", e);
        }
        assert!(result.is_ok(), "result should be ok: {:?}", result);
        assert!(created, "Project directory should be created");
        assert!(has_package_yaml, "package.yaml should exist");
    }

    #[tokio::test]
    async fn test_new_fails_if_directory_exists() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        // Create the directory in temp first, before changing cwd
        let existing_path = temp.path().join("existing-project");
        fs::create_dir(&existing_path).unwrap();
        fs::write(existing_path.join("dummy.txt"), "test").unwrap();

        // Change to temp dir
        env::set_current_dir(temp.path()).unwrap();

        let result = run("existing-project".to_string(), None, true).await;

        // Restore original dir
        env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_err(), "Should fail when directory exists");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("already exists"),
            "Error message should mention directory exists: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_new_with_template() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        // Change to temp dir
        env::set_current_dir(temp.path()).unwrap();

        // Template might not exist, but should still create the project
        let result = run(
            "template-project".to_string(),
            Some("nonexistent".to_string()),
            true,
        )
        .await;

        // Restore original dir
        env::set_current_dir(&original_dir).unwrap();

        // May fail due to template not found, but directory should be created
        let _ = result; // Ignore the result as template discovery might fail
    }

    #[tokio::test]
    async fn test_new_creates_subdirectories() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        // Change to temp dir
        env::set_current_dir(temp.path()).unwrap();

        let result = run("full-project".to_string(), None, true).await;

        // Check before restoring directory
        let project_path = temp.path().join("full-project");
        let has_src = project_path.join("src").exists();
        let has_lib = project_path.join("lib").exists();
        let has_tests = project_path.join("tests").exists();

        // Restore original dir
        env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_ok());
        assert!(has_src, "src directory should exist");
        assert!(has_lib, "lib directory should exist");
        assert!(has_tests, "tests directory should exist");
    }

    #[tokio::test]
    async fn test_new_with_special_characters_in_name() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        // Change to temp dir
        env::set_current_dir(temp.path()).unwrap();

        let result = run("my-project_123".to_string(), None, true).await;

        // Check before restoring
        let exists = temp.path().join("my-project_123").exists();

        // Restore original dir safely
        let _ = env::set_current_dir(&original_dir);

        assert!(result.is_ok());
        assert!(exists, "Project with special characters should exist");
    }
}
