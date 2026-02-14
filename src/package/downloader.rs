//! GitHub package downloader with parallel download support

use crate::core::{DepotError, DepotResult};
use crate::di::traits::GitHubProvider;
use crate::github::types::ResolvedVersion;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinSet;

/// Information about a package to download from GitHub
#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub repository: String,                // "owner/repo"
    pub version: Option<String>,           // Optional version constraint
    pub resolved: Option<ResolvedVersion>, // Pre-resolved version (skip resolution if provided)
}

/// Result of a download operation
#[derive(Debug)]
pub struct DownloadResult {
    pub repository: String,
    pub resolved: ResolvedVersion,
    pub tarball_path: PathBuf,
    pub error: Option<DepotError>,
}

/// Manages parallel/concurrent package downloads from GitHub
pub struct ParallelDownloader {
    github: Arc<dyn GitHubProvider>,
    max_concurrent: usize,
    fallback_chain: Vec<String>,
}

impl ParallelDownloader {
    /// Create a new parallel downloader
    pub fn new(
        github: Arc<dyn GitHubProvider>,
        fallback_chain: Vec<String>,
        max_concurrent: Option<usize>,
    ) -> Self {
        Self {
            github,
            max_concurrent: max_concurrent.unwrap_or(10),
            fallback_chain,
        }
    }

    /// Download multiple packages in parallel
    pub async fn download_packages(&self, tasks: Vec<DownloadTask>) -> Vec<DownloadResult> {
        let mut results = Vec::new();
        let mut join_set = JoinSet::new();

        for task in tasks {
            // Wait if we've hit the concurrency limit
            if join_set.len() >= self.max_concurrent {
                if let Some(Ok(download_result)) = join_set.join_next().await {
                    results.push(download_result);
                }
            }

            let github = Arc::clone(&self.github);
            let fallback_chain = self.fallback_chain.clone();
            join_set.spawn(async move {
                Self::download_single_package(github.as_ref(), task, &fallback_chain).await
            });
        }

        // Wait for all remaining tasks
        while let Some(result) = join_set.join_next().await {
            if let Ok(download_result) = result {
                results.push(download_result);
            }
        }

        results
    }

    /// Download a single package from GitHub
    async fn download_single_package(
        github: &dyn GitHubProvider,
        task: DownloadTask,
        fallback_chain: &[String],
    ) -> DownloadResult {
        let repository = task.repository.clone();

        // Parse owner/repo
        let parts: Vec<&str> = repository.split('/').collect();
        if parts.len() != 2 {
            return DownloadResult {
                repository: repository.clone(),
                resolved: ResolvedVersion {
                    ref_type: crate::github::types::RefType::Branch,
                    ref_value: String::new(),
                    commit_sha: String::new(),
                    tarball_url: String::new(),
                },
                tarball_path: PathBuf::new(),
                error: Some(DepotError::Config(format!(
                    "Invalid repository format '{}'. Expected 'owner/repo'",
                    repository
                ))),
            };
        }

        let owner = parts[0];
        let repo = parts[1];

        // Resolve version if not already resolved
        let resolved = if let Some(resolved) = task.resolved {
            resolved
        } else {
            match github
                .resolve_version(owner, repo, task.version.as_deref(), fallback_chain)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    return DownloadResult {
                        repository: repository.clone(),
                        resolved: ResolvedVersion {
                            ref_type: crate::github::types::RefType::Branch,
                            ref_value: String::new(),
                            commit_sha: String::new(),
                            tarball_url: String::new(),
                        },
                        tarball_path: PathBuf::new(),
                        error: Some(e),
                    };
                }
            }
        };

        // Download tarball
        match github
            .download_tarball(owner, repo, &resolved.ref_value)
            .await
        {
            Ok(tarball_path) => DownloadResult {
                repository,
                resolved,
                tarball_path,
                error: None,
            },
            Err(e) => DownloadResult {
                repository,
                resolved,
                tarball_path: PathBuf::new(),
                error: Some(e),
            },
        }
    }

    /// Download packages with progress reporting
    pub async fn download_with_progress(
        &self,
        tasks: Vec<DownloadTask>,
    ) -> DepotResult<Vec<DownloadResult>> {
        let total = tasks.len();

        if total == 0 {
            return Ok(Vec::new());
        }

        // Create progress bar
        let pb = ProgressBar::new(total as u64);
        if let Ok(style) = ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} packages")
        {
            pb.set_style(style.progress_chars("#>-"));
        }

        // Download packages
        let results = self.download_packages(tasks).await;

        // Update progress bar and report results
        let mut error_count = 0;

        for result in &results {
            pb.inc(1);
            if result.error.is_none() {
                pb.println(format!("  ✓ {}", result.repository));
            } else {
                error_count += 1;
                if let Some(ref error) = result.error {
                    pb.println(format!("  ✗ {} (error: {})", result.repository, error));
                }
            }
        }

        pb.finish_with_message("Download complete");

        if error_count > 0 {
            return Err(DepotError::Package(format!(
                "Failed to download {} package(s)",
                error_count
            )));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::mocks::MockGitHubProvider;
    use crate::github::types::{RefType, ResolvedVersion};
    use crate::github::GitHubRelease;

    #[test]
    fn test_download_task_clone() {
        let task = DownloadTask {
            repository: "owner/repo".to_string(),
            version: Some("1.0.0".to_string()),
            resolved: None,
        };
        let cloned = task.clone();
        assert_eq!(task.repository, cloned.repository);
        assert_eq!(task.version, cloned.version);
    }

    #[test]
    fn test_parallel_downloader_new() {
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string(), "tag".to_string()];
        let downloader = ParallelDownloader::new(github, fallback, Some(5));
        assert_eq!(downloader.max_concurrent, 5);
    }

    #[test]
    fn test_parallel_downloader_default_concurrency() {
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];
        let downloader = ParallelDownloader::new(github, fallback, None);
        assert_eq!(downloader.max_concurrent, 10);
    }

    #[tokio::test]
    async fn test_download_empty_tasks() {
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];
        let downloader = ParallelDownloader::new(github, fallback, None);

        let results = downloader.download_packages(vec![]).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_download_with_progress_empty() {
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];
        let downloader = ParallelDownloader::new(github, fallback, None);

        let results = downloader.download_with_progress(vec![]).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_download_single_package_invalid_repository() {
        let github = MockGitHubProvider::new();
        let fallback = vec!["release".to_string()];

        let task = DownloadTask {
            repository: "invalid".to_string(),
            version: None,
            resolved: None,
        };

        let result = ParallelDownloader::download_single_package(&github, task, &fallback).await;

        assert!(result.error.is_some());
        assert!(result
            .error
            .unwrap()
            .to_string()
            .contains("Invalid repository format"));
    }

    #[tokio::test]
    async fn test_download_single_package_with_preresolved() {
        let github = MockGitHubProvider::new();
        let fallback = vec!["release".to_string()];

        // Add a tarball to the mock
        github.add_tarball(
            "owner",
            "repo",
            "v1.0.0",
            PathBuf::from("/tmp/test-tarball.tar.gz"),
        );

        let resolved = ResolvedVersion {
            ref_type: RefType::Release,
            ref_value: "v1.0.0".to_string(),
            commit_sha: "abc123".to_string(),
            tarball_url: "https://api.github.com/repos/owner/repo/tarball/v1.0.0".to_string(),
        };

        let task = DownloadTask {
            repository: "owner/repo".to_string(),
            version: None,
            resolved: Some(resolved),
        };

        let result = ParallelDownloader::download_single_package(&github, task, &fallback).await;

        assert!(result.error.is_none());
        assert_eq!(result.repository, "owner/repo");
    }

    #[tokio::test]
    async fn test_download_packages_with_concurrency_limit() {
        let github = Arc::new(MockGitHubProvider::new());

        // Add multiple tarballs
        for i in 0..5 {
            let repo_name = format!("repo{}", i);
            github.add_tarball(
                "owner",
                &repo_name,
                "v1.0.0",
                PathBuf::from(format!("/tmp/test-tarball-{}.tar.gz", i)),
            );
            github.add_release(
                "owner",
                &repo_name,
                GitHubRelease {
                    tag_name: "v1.0.0".to_string(),
                    name: Some(format!("Release v1.0.0 for repo{}", i)),
                    tarball_url: format!(
                        "https://api.github.com/repos/owner/repo{}/tarball/v1.0.0",
                        i
                    ),
                    zipball_url: format!(
                        "https://api.github.com/repos/owner/repo{}/zipball/v1.0.0",
                        i
                    ),
                    prerelease: false,
                    draft: false,
                    assets: Vec::new(),
                    body: Some("Test release".to_string()),
                    published_at: Some("2024-01-01T00:00:00Z".to_string()),
                },
            );
        }

        let fallback = vec!["release".to_string()];
        let downloader = ParallelDownloader::new(github, fallback, Some(2));

        let tasks: Vec<DownloadTask> = (0..5)
            .map(|i| DownloadTask {
                repository: format!("owner/repo{}", i),
                version: Some("v1.0.0".to_string()),
                resolved: None,
            })
            .collect();

        let results = downloader.download_packages(tasks).await;
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_download_with_progress_all_failures() {
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];
        let downloader = ParallelDownloader::new(github, fallback, None);

        let tasks = vec![
            DownloadTask {
                repository: "owner/nonexistent1".to_string(),
                version: None,
                resolved: None,
            },
            DownloadTask {
                repository: "owner/nonexistent2".to_string(),
                version: None,
                resolved: None,
            },
        ];

        let result = downloader.download_with_progress(tasks).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_task_without_version() {
        let task = DownloadTask {
            repository: "owner/repo".to_string(),
            version: None,
            resolved: None,
        };
        assert!(task.version.is_none());
    }

    #[tokio::test]
    async fn test_download_result_debug() {
        let result = DownloadResult {
            repository: "owner/repo".to_string(),
            resolved: ResolvedVersion {
                ref_type: RefType::Release,
                ref_value: "v1.0.0".to_string(),
                commit_sha: "abc123".to_string(),
                tarball_url: "https://api.github.com/repos/owner/repo/tarball/v1.0.0".to_string(),
            },
            tarball_path: PathBuf::from("/tmp/test.tar.gz"),
            error: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("owner/repo"));
    }
}
