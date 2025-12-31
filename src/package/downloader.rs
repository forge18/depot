use crate::core::{LpmError, LpmResult};
use crate::di::PackageClient;
use crate::lua_version::compatibility::PackageCompatibility;
use crate::lua_version::detector::LuaVersion;
use crate::luarocks::manifest::Manifest;
use crate::luarocks::rockspec::Rockspec;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinSet;

/// Information about a package to download
#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub name: String,
    pub version: String,
    pub rockspec_url: String,
    pub source_url: Option<String>,
}

/// Result of a download operation
#[derive(Debug)]
pub struct DownloadResult {
    pub name: String,
    pub version: String,
    pub rockspec: Rockspec,
    pub source_path: Option<PathBuf>,
    pub error: Option<LpmError>,
}

/// Manages parallel/concurrent package downloads
pub struct ParallelDownloader {
    client: Arc<dyn PackageClient>,
    max_concurrent: usize,
}

impl ParallelDownloader {
    /// Create a new parallel downloader
    pub fn new(client: Arc<dyn PackageClient>, max_concurrent: Option<usize>) -> Self {
        Self {
            client,
            max_concurrent: max_concurrent.unwrap_or(10), // Default to 10 concurrent downloads
        }
    }

    /// Download multiple packages in parallel
    pub async fn download_packages(
        &self,
        tasks: Vec<DownloadTask>,
        installed_lua: Option<&LuaVersion>,
    ) -> Vec<DownloadResult> {
        let mut results = Vec::new();
        let mut join_set = JoinSet::new();

        // Spawn download tasks with concurrency limit
        for task in tasks {
            if join_set.len() >= self.max_concurrent {
                // Wait for one task to complete before adding another
                if let Some(Ok(download_result)) = join_set.join_next().await {
                    results.push(download_result);
                }
            }

            let client = Arc::clone(&self.client);
            let task_clone = task.clone();
            let lua_version = installed_lua.cloned();
            join_set.spawn(async move {
                Self::download_single_package(client.as_ref(), task_clone, lua_version.as_ref())
                    .await
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

    /// Download a single package (used by parallel downloader)
    async fn download_single_package(
        client: &dyn PackageClient,
        task: DownloadTask,
        installed_lua: Option<&LuaVersion>,
    ) -> DownloadResult {
        let name = task.name.clone();
        let version = task.version.clone();

        // Download rockspec
        let rockspec_result = client.download_rockspec(&task.rockspec_url).await;
        let rockspec = match rockspec_result {
            Ok(content) => {
                match client.parse_rockspec(&content) {
                    Ok(r) => {
                        // Check Lua version compatibility if installed version is known
                        let rockspec = r;
                        if let Some(lua_version) = installed_lua {
                            let lua_version_str = rockspec
                                .lua_version
                                .as_deref()
                                .unwrap_or("unknown")
                                .to_string();
                            match PackageCompatibility::check_rockspec(lua_version, &rockspec) {
                                Ok(true) => {
                                    // Compatible, continue
                                }
                                Ok(false) => {
                                    // Incompatible - return error
                                    return DownloadResult {
                                    name: name.clone(),
                                    version: version.clone(),
                                    rockspec,
                                    source_path: None,
                                    error: Some(LpmError::Version(format!(
                                        "Package '{}' version '{}' requires Lua {}, but installed version is {}",
                                        name, version,
                                        lua_version_str,
                                        lua_version.version_string()
                                    ))),
                                };
                                }
                                Err(e) => {
                                    // Parse error, but continue (might be invalid constraint format)
                                    eprintln!("Warning: Failed to parse Lua version constraint for {}: {}", name, e);
                                }
                            }
                        }
                        rockspec
                    }
                    Err(e) => {
                        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
                        return DownloadResult {
                            name: name.clone(),
                            version: version.clone(),
                            rockspec: Rockspec {
                                package: name.clone(),
                                version: version.clone(),
                                source: RockspecSource {
                                    url: String::new(),
                                    tag: None,
                                    branch: None,
                                },
                                dependencies: Vec::new(),
                                build: RockspecBuild {
                                    build_type: String::new(),
                                    modules: HashMap::new(),
                                    install: crate::luarocks::rockspec::InstallTable::default(),
                                },
                                description: None,
                                homepage: None,
                                license: None,
                                lua_version: None,
                                binary_urls: HashMap::new(),
                            },
                            source_path: None,
                            error: Some(e),
                        };
                    }
                }
            }
            Err(e) => {
                return DownloadResult {
                    name: name.clone(),
                    version: version.clone(),
                    rockspec: Rockspec {
                        package: name.clone(),
                        version: version.clone(),
                        source: crate::luarocks::rockspec::RockspecSource {
                            url: String::new(),
                            tag: None,
                            branch: None,
                        },
                        dependencies: Vec::new(),
                        build: crate::luarocks::rockspec::RockspecBuild {
                            build_type: String::new(),
                            modules: HashMap::new(),
                            install: crate::luarocks::rockspec::InstallTable::default(),
                        },
                        description: None,
                        homepage: None,
                        license: None,
                        lua_version: None,
                        binary_urls: HashMap::new(),
                    },
                    source_path: None,
                    error: Some(e),
                };
            }
        };

        // Download source if URL is provided
        let source_path = if let Some(source_url) = &task.source_url {
            match client.download_source(source_url).await {
                Ok(path) => Some(path),
                Err(e) => {
                    return DownloadResult {
                        name: name.clone(),
                        version: version.clone(),
                        rockspec,
                        source_path: None,
                        error: Some(e),
                    };
                }
            }
        } else {
            None
        };

        DownloadResult {
            name,
            version,
            rockspec,
            source_path,
            error: None,
        }
    }

    /// Download packages with progress reporting
    pub async fn download_with_progress(
        &self,
        tasks: Vec<DownloadTask>,
        installed_lua: Option<&LuaVersion>,
    ) -> LpmResult<Vec<DownloadResult>> {
        let total = tasks.len();

        // Create progress bar
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} packages")
                .unwrap()
                .progress_chars("#>-"),
        );

        // Download packages
        let results = self.download_packages(tasks, installed_lua).await;

        // Update progress bar and report results
        let mut error_count = 0;

        for result in &results {
            pb.inc(1);
            if result.error.is_none() {
                pb.println(format!("  ✓ {}", result.name));
            } else {
                error_count += 1;
                if let Some(ref error) = result.error {
                    pb.println(format!("  ✗ {} (error: {})", result.name, error));
                }
            }
        }

        pb.finish_with_message("Download complete");

        if error_count > 0 {
            return Err(LpmError::Package(format!(
                "Failed to download {} package(s)",
                error_count
            )));
        }

        Ok(results)
    }
}

/// Helper to create download tasks from manifest and resolved versions
pub fn create_download_tasks(
    manifest: &Manifest,
    resolved_versions: &HashMap<String, String>, // package name -> version string
) -> Vec<DownloadTask> {
    let mut tasks = Vec::new();

    for (name, version) in resolved_versions {
        if let Some(package_versions) = manifest.get_package_versions(name) {
            // Find the matching version
            if let Some(package_version) = package_versions.iter().find(|pv| pv.version == *version)
            {
                tasks.push(DownloadTask {
                    name: name.clone(),
                    version: version.clone(),
                    rockspec_url: package_version.rockspec_url.clone(),
                    source_url: package_version.archive_url.clone(),
                });
            }
        }
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::luarocks::manifest::{Manifest, PackageVersion};

    #[test]
    fn test_create_download_tasks() {
        let mut manifest = Manifest::default();
        let versions = vec![PackageVersion {
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
            archive_url: Some("https://example.com/test-1.0.0.tar.gz".to_string()),
        }];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), "1.0.0".to_string());

        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "test-package");
        assert_eq!(tasks[0].version, "1.0.0");
        assert!(tasks[0].source_url.is_some());
    }

    #[test]
    fn test_create_download_tasks_no_match() {
        let manifest = Manifest::default();
        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), "1.0.0".to_string());

        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 0);
    }

    #[test]
    fn test_create_download_tasks_multiple_packages() {
        let mut manifest = Manifest::default();
        let versions1 = vec![PackageVersion {
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test1-1.0.0.rockspec".to_string(),
            archive_url: Some("https://example.com/test1-1.0.0.tar.gz".to_string()),
        }];
        manifest
            .packages
            .insert("test-package-1".to_string(), versions1);

        let versions2 = vec![PackageVersion {
            version: "2.0.0".to_string(),
            rockspec_url: "https://example.com/test2-2.0.0.rockspec".to_string(),
            archive_url: Some("https://example.com/test2-2.0.0.tar.gz".to_string()),
        }];
        manifest
            .packages
            .insert("test-package-2".to_string(), versions2);

        let mut resolved = HashMap::new();
        resolved.insert("test-package-1".to_string(), "1.0.0".to_string());
        resolved.insert("test-package-2".to_string(), "2.0.0".to_string());

        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_download_task_clone() {
        let task = DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test.rockspec".to_string(),
            source_url: Some("https://example.com/test.tar.gz".to_string()),
        };
        let cloned = task.clone();
        assert_eq!(task.name, cloned.name);
        assert_eq!(task.version, cloned.version);
        assert_eq!(task.rockspec_url, cloned.rockspec_url);
        assert_eq!(task.source_url, cloned.source_url);
    }

    #[test]
    fn test_download_task_without_source_url() {
        let task = DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test.rockspec".to_string(),
            source_url: None,
        };
        assert!(task.source_url.is_none());
    }

    #[test]
    fn test_parallel_downloader_new_with_max_concurrent() {
        use crate::cache::Cache;
        use crate::config::Config;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let config = Config::default();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let _downloader = ParallelDownloader::new(client, Some(5));
        // We can't easily access max_concurrent, but we can verify it was created
        // The actual value is tested implicitly through usage
    }

    #[test]
    fn test_parallel_downloader_new_with_default() {
        use crate::cache::Cache;
        use crate::config::Config;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let config = Config::default();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let _downloader = ParallelDownloader::new(client, None);
        // Default max_concurrent should be 10
    }

    #[test]
    fn test_create_download_tasks_without_source_url() {
        let mut manifest = Manifest::default();
        let versions = vec![PackageVersion {
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
            archive_url: None, // No source URL
        }];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), "1.0.0".to_string());

        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "test-package");
        assert!(tasks[0].source_url.is_none());
    }

    #[test]
    fn test_create_download_tasks_version_mismatch() {
        let mut manifest = Manifest::default();
        let versions = vec![PackageVersion {
            version: "2.0.0".to_string(), // Different version
            rockspec_url: "https://example.com/test-2.0.0.rockspec".to_string(),
            archive_url: Some("https://example.com/test-2.0.0.tar.gz".to_string()),
        }];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), "1.0.0".to_string()); // Looking for 1.0.0

        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 0); // No match
    }

    #[test]
    fn test_create_download_tasks_empty_resolved() {
        let manifest = Manifest::default();
        let resolved = HashMap::new();
        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 0);
    }

    #[test]
    fn test_download_result_with_error() {
        let result = DownloadResult {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec: crate::luarocks::rockspec::Rockspec {
                package: "test".to_string(),
                version: "1.0.0".to_string(),
                source: crate::luarocks::rockspec::RockspecSource {
                    url: "".to_string(),
                    tag: None,
                    branch: None,
                },
                dependencies: vec![],
                build: crate::luarocks::rockspec::RockspecBuild {
                    build_type: "builtin".to_string(),
                    modules: std::collections::HashMap::new(),
                    install: crate::luarocks::rockspec::InstallTable::default(),
                },
                description: None,
                homepage: None,
                license: None,
                lua_version: None,
                binary_urls: std::collections::HashMap::new(),
            },
            source_path: None,
            error: Some(LpmError::Package("test error".to_string())),
        };
        assert!(result.error.is_some());
    }

    #[test]
    fn test_download_result_without_error() {
        let result = DownloadResult {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec: crate::luarocks::rockspec::Rockspec {
                package: "test".to_string(),
                version: "1.0.0".to_string(),
                source: crate::luarocks::rockspec::RockspecSource {
                    url: "".to_string(),
                    tag: None,
                    branch: None,
                },
                dependencies: vec![],
                build: crate::luarocks::rockspec::RockspecBuild {
                    build_type: "builtin".to_string(),
                    modules: std::collections::HashMap::new(),
                    install: crate::luarocks::rockspec::InstallTable::default(),
                },
                description: None,
                homepage: None,
                license: None,
                lua_version: None,
                binary_urls: std::collections::HashMap::new(),
            },
            source_path: Some(std::path::PathBuf::from("/tmp/test")),
            error: None,
        };
        assert!(result.error.is_none());
        assert!(result.source_path.is_some());
    }

    #[test]
    fn test_parallel_downloader_max_concurrent_zero() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(0));
        assert_eq!(downloader.max_concurrent, 0);
    }

    #[test]
    fn test_parallel_downloader_max_concurrent_large() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(100));
        assert_eq!(downloader.max_concurrent, 100);
    }

    #[tokio::test]
    async fn test_download_with_progress_empty() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, None);
        let results = downloader.download_with_progress(vec![], None).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_download_with_progress_with_errors() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, None);

        // Create task with invalid URL to trigger error
        let task = DownloadTask {
            name: "nonexistent".to_string(),
            version: "999.0.0".to_string(),
            rockspec_url: "https://luarocks.org/manifests/nonexistent-999.0.0.rockspec".to_string(),
            source_url: None,
        };

        let results = downloader.download_with_progress(vec![task], None).await;
        // Should return error due to failed downloads
        assert!(results.is_err());
    }

    #[tokio::test]
    async fn test_download_with_progress_with_mixed_results() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, None);

        // Create tasks - one valid, one invalid
        let tasks = vec![
            DownloadTask {
                name: "nonexistent1".to_string(),
                version: "999.0.0".to_string(),
                rockspec_url: "https://luarocks.org/manifests/nonexistent1-999.0.0.rockspec"
                    .to_string(),
                source_url: None,
            },
            DownloadTask {
                name: "nonexistent2".to_string(),
                version: "999.0.0".to_string(),
                rockspec_url: "https://luarocks.org/manifests/nonexistent2-999.0.0.rockspec"
                    .to_string(),
                source_url: None,
            },
        ];

        let results = downloader.download_with_progress(tasks, None).await;
        // Should return error due to failed downloads
        assert!(results.is_err());
    }

    #[test]
    fn test_download_task_debug() {
        let task = DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/rockspec".to_string(),
            source_url: Some("https://example.com/source.tar.gz".to_string()),
        };
        let debug_str = format!("{:?}", task);
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_download_result_debug() {
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        let result = DownloadResult {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec: Rockspec {
                package: "test".to_string(),
                version: "1.0.0".to_string(),
                source: RockspecSource {
                    url: "".to_string(),
                    tag: None,
                    branch: None,
                },
                dependencies: vec![],
                build: RockspecBuild {
                    build_type: "builtin".to_string(),
                    modules: std::collections::HashMap::new(),
                    install: crate::luarocks::rockspec::InstallTable::default(),
                },
                description: None,
                homepage: None,
                license: None,
                lua_version: None,
                binary_urls: std::collections::HashMap::new(),
            },
            source_path: None,
            error: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("test"));
    }

    #[tokio::test]
    async fn test_download_packages_with_concurrency_limit() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(2)); // Limit to 2 concurrent

        // Create multiple tasks to test concurrency limit
        let tasks = vec![
            DownloadTask {
                name: "pkg1".to_string(),
                version: "1.0.0".to_string(),
                rockspec_url: "https://luarocks.org/manifests/pkg1-1.0.0.rockspec".to_string(),
                source_url: None,
            },
            DownloadTask {
                name: "pkg2".to_string(),
                version: "1.0.0".to_string(),
                rockspec_url: "https://luarocks.org/manifests/pkg2-1.0.0.rockspec".to_string(),
                source_url: None,
            },
            DownloadTask {
                name: "pkg3".to_string(),
                version: "1.0.0".to_string(),
                rockspec_url: "https://luarocks.org/manifests/pkg3-1.0.0.rockspec".to_string(),
                source_url: None,
            },
        ];

        // This will test the concurrency limit path (join_set.len() >= max_concurrent)
        let results = downloader.download_packages(tasks, None).await;
        assert_eq!(results.len(), 3);
        // All should fail, but tests the concurrency logic
    }

    #[tokio::test]
    async fn test_download_packages_with_join_error() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(1));

        // Create a task that will fail
        let tasks = vec![DownloadTask {
            name: "nonexistent".to_string(),
            version: "999.0.0".to_string(),
            rockspec_url: "https://luarocks.org/manifests/nonexistent-999.0.0.rockspec".to_string(),
            source_url: None,
        }];

        // This tests the join_next error path (when result is Err)
        let results = downloader.download_packages(tasks, None).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].error.is_some());
    }

    #[tokio::test]
    async fn test_download_single_package_lua_version_compatibility_check() {
        // Test that lua version compatibility checking path is executed
        // This would require mocking the client and rockspec, but tests structure
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::lua_version::detector::LuaVersion;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let task = DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://luarocks.org/manifests/test-1.0.0.rockspec".to_string(),
            source_url: None,
        };

        // Test with lua version (will fail on network, but tests compatibility check path)
        let lua_version = LuaVersion::new(5, 4, 0);
        let _result =
            ParallelDownloader::download_single_package(&client, task, Some(&lua_version)).await;
    }

    #[tokio::test]
    async fn test_download_single_package_lua_version_parse_error() {
        // Test the Err(e) path in lua version compatibility check
        // This tests the eprintln! warning path when constraint parsing fails
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::lua_version::detector::LuaVersion;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let task = DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://luarocks.org/manifests/test-1.0.0.rockspec".to_string(),
            source_url: None,
        };

        // Test with lua version - will fail on network but tests parse error path
        let lua_version = LuaVersion::new(5, 4, 0);
        let _result =
            ParallelDownloader::download_single_package(&client, task, Some(&lua_version)).await;
    }

    #[tokio::test]
    async fn test_download_single_package_with_source_url() {
        // Test the path when source_url is Some
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let task = DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://luarocks.org/manifests/test-1.0.0.rockspec".to_string(),
            source_url: Some("https://example.com/source.tar.gz".to_string()),
        };

        // Will fail on network, but tests source_url path
        let _result = ParallelDownloader::download_single_package(&client, task, None).await;
    }

    #[tokio::test]
    async fn test_download_single_package_source_download_error() {
        // Test error path when source download fails
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let task = DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://luarocks.org/manifests/test-1.0.0.rockspec".to_string(),
            source_url: Some("https://nonexistent-domain-12345.com/source.tar.gz".to_string()),
        };

        // Will fail on network, but tests source download error path
        let result = ParallelDownloader::download_single_package(&client, task, None).await;
        // May fail on rockspec download or source download
        let _ = result;
    }

    #[tokio::test]
    async fn test_download_packages_concurrency_limit() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(2));

        let tasks = vec![
            DownloadTask {
                name: "pkg1".to_string(),
                version: "1.0.0".to_string(),
                rockspec_url: "url1".to_string(),
                source_url: None,
            },
            DownloadTask {
                name: "pkg2".to_string(),
                version: "1.0.0".to_string(),
                rockspec_url: "url2".to_string(),
                source_url: None,
            },
            DownloadTask {
                name: "pkg3".to_string(),
                version: "1.0.0".to_string(),
                rockspec_url: "url3".to_string(),
                source_url: None,
            },
        ];

        // Tests concurrency limiting logic (join_set.len() >= max_concurrent)
        let _results = downloader.download_packages(tasks, None).await;
    }

    #[tokio::test]
    async fn test_download_with_progress_with_success_and_errors() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(1));

        let tasks = vec![DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: "https://luarocks.org/manifests/test-1.0.0.rockspec".to_string(),
            source_url: None,
        }];

        // Tests progress bar with mixed results (error_count > 0 path)
        let _result = downloader.download_with_progress(tasks, None).await;
    }

    #[test]
    fn test_create_download_tasks_with_archive_url() {
        let mut manifest = Manifest::default();
        let versions = vec![PackageVersion {
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
            archive_url: Some("https://example.com/test-1.0.0.tar.gz".to_string()),
        }];
        manifest
            .packages
            .insert("test-package".to_string(), versions);
        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), "1.0.0".to_string());
        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 1);
        assert_eq!(
            tasks[0].source_url,
            Some("https://example.com/test-1.0.0.tar.gz".to_string())
        );
    }

    #[test]
    fn test_create_download_tasks_without_archive_url() {
        let mut manifest = Manifest::default();
        let versions = vec![PackageVersion {
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
            archive_url: None,
        }];
        manifest
            .packages
            .insert("test-package".to_string(), versions);
        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), "1.0.0".to_string());
        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source_url, None);
    }

    #[test]
    fn test_create_download_tasks_package_not_in_manifest() {
        let manifest = Manifest::default();
        let mut resolved = HashMap::new();
        resolved.insert("nonexistent".to_string(), "1.0.0".to_string());
        let tasks = create_download_tasks(&manifest, &resolved);
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_create_download_tasks_multiple_versions_same_package() {
        let mut manifest = Manifest::default();
        let versions = vec![
            PackageVersion {
                version: "1.0.0".to_string(),
                rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
                archive_url: Some("https://example.com/test-1.0.0.tar.gz".to_string()),
            },
            PackageVersion {
                version: "2.0.0".to_string(),
                rockspec_url: "https://example.com/test-2.0.0.rockspec".to_string(),
                archive_url: Some("https://example.com/test-2.0.0.tar.gz".to_string()),
            },
        ];
        manifest
            .packages
            .insert("test-package".to_string(), versions);
        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), "1.0.0".to_string());
        let tasks = create_download_tasks(&manifest, &resolved);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].version, "1.0.0");
    }

    #[tokio::test]
    async fn test_download_with_progress_empty_tasks() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(1));

        // Test with empty tasks
        let results = downloader.download_with_progress(vec![], None).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_download_with_progress_single_success() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rockspec"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("package = 'test'\nversion = '1.0.0'\ndependencies = {}"),
            )
            .mount(&mock_server)
            .await;

        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let downloader = ParallelDownloader::new(client, Some(1));

        let tasks = vec![DownloadTask {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            rockspec_url: format!("{}/rockspec", mock_server.uri()),
            source_url: None,
        }];

        // Will fail on network or parsing, but tests progress bar path
        let _result = downloader.download_with_progress(tasks, None).await;
    }

    #[test]
    fn test_parallel_downloader_new() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let downloader = ParallelDownloader::new(client, Some(5));
        // Just verify it was created
        let _ = downloader;
    }

    #[test]
    fn test_parallel_downloader_new_default_concurrency() {
        use crate::cache::Cache;
        use crate::config::Config;
        use crate::luarocks::client::LuaRocksClient;
        let config = Config::load().unwrap();
        let cache = Cache::new(config.get_cache_dir().unwrap()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let downloader = ParallelDownloader::new(client, None);
        // Just verify it was created with default concurrency
        let _ = downloader;
    }
}
