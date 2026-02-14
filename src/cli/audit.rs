use depot::core::path::find_project_root;
use depot::core::{DepotError, DepotResult};
use depot::security::audit::{format_report, SecurityAuditor};
use std::env;
use std::path::Path;

pub async fn run() -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;
    run_in_dir(&current_dir).await
}

pub async fn run_in_dir(dir: &Path) -> DepotResult<()> {
    let project_root = find_project_root(dir)?;

    println!("Running security audit...");
    println!("  Querying OSV (Open Source Vulnerabilities) database...");
    println!();

    let report = SecurityAuditor::audit_project_with_osv(&project_root).await?;

    // Display results
    let output = format_report(&report);
    print!("{}", output);

    // Return error if critical/high vulnerabilities found
    if report.has_critical() || report.has_high() {
        let severity = if report.has_critical() {
            "critical"
        } else {
            "high"
        };
        return Err(DepotError::AuditFailed(format!(
            "Found {} severity vulnerabilities",
            severity
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use depot::package::lockfile::Lockfile;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_run_error_no_lockfile() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\n",
        )
        .unwrap();

        let result = run_in_dir(temp.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No depot.lock"));
    }

    #[tokio::test]
    async fn test_run_error_no_project_root() {
        let temp = TempDir::new().unwrap();
        let subdir = temp.path().join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = run_in_dir(&subdir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_with_empty_lockfile() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\n",
        )
        .unwrap();

        let lockfile = Lockfile::new();
        lockfile.save(temp.path()).unwrap();

        let result = run_in_dir(temp.path()).await;
        // Should succeed with empty lockfile (no packages to check)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_function_exists() {
        let _ = run;
    }

    #[test]
    fn test_run_in_dir_function_exists() {
        let _ = run_in_dir;
    }

    #[tokio::test]
    async fn test_run_in_dir_no_project() {
        let temp = TempDir::new().unwrap();
        let subdir = temp.path().join("not-a-project");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = run_in_dir(&subdir).await;
        assert!(result.is_err());
    }
}
