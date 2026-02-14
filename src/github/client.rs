//! GitHub API client implementation

use crate::config::Config;
use crate::core::{DepotError, DepotResult};
use crate::di::traits::{CacheProvider, GitHubProvider};
use crate::github::types::{GitHubRelease, GitHubRepo, GitHubTag, RefType, ResolvedVersion};
use async_trait::async_trait;
use reqwest::{header, Client as HttpClient};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;

/// GitHub API client
pub struct GitHubClient {
    http_client: HttpClient,
    api_url: String,
    token: Option<String>,
    rate_limiter: Arc<RateLimiter>,
    cache: Arc<dyn CacheProvider>,
}

/// Rate limiter for GitHub API
struct RateLimiter {
    remaining: Mutex<u64>,
    reset_time: Mutex<SystemTime>,
}

impl GitHubClient {
    /// Create a new GitHub client
    pub fn new(config: &Config, cache: Arc<dyn CacheProvider>) -> DepotResult<Self> {
        let token = std::env::var("GITHUB_TOKEN")
            .ok()
            .or_else(|| config.github.token.clone());

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("depot-package-manager"),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/vnd.github.v3+json"),
        );

        if let Some(ref token) = token {
            headers.insert(
                header::AUTHORIZATION,
                header::HeaderValue::from_str(&format!("token {}", token))
                    .map_err(|e| DepotError::Config(format!("Invalid GitHub token: {}", e)))?,
            );
        }

        let http_client = HttpClient::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| DepotError::Package(format!("Failed to create HTTP client: {}", e)))?;

        let has_token = token.is_some();
        Ok(Self {
            http_client,
            api_url: config.github.api_url.clone(),
            token,
            rate_limiter: Arc::new(RateLimiter {
                remaining: Mutex::new(if has_token { 5000 } else { 60 }),
                reset_time: Mutex::new(SystemTime::now() + Duration::from_secs(3600)),
            }),
            cache,
        })
    }

    /// Get releases for a repository
    pub async fn get_releases(&self, owner: &str, repo: &str) -> DepotResult<Vec<GitHubRelease>> {
        let url = format!("{}/repos/{}/{}/releases", self.api_url, owner, repo);
        self.api_get(&url).await
    }

    /// Get the latest release for a repository
    pub async fn get_latest_release(&self, owner: &str, repo: &str) -> DepotResult<GitHubRelease> {
        let url = format!("{}/repos/{}/{}/releases/latest", self.api_url, owner, repo);
        self.api_get(&url).await
    }

    /// Get tags for a repository
    pub async fn get_tags(&self, owner: &str, repo: &str) -> DepotResult<Vec<GitHubTag>> {
        let url = format!("{}/repos/{}/{}/tags", self.api_url, owner, repo);
        self.api_get(&url).await
    }

    /// Get repository information
    pub async fn get_repo(&self, owner: &str, repo: &str) -> DepotResult<GitHubRepo> {
        let url = format!("{}/repos/{}/{}", self.api_url, owner, repo);
        self.api_get(&url).await
    }

    /// Get the default branch for a repository
    pub async fn get_default_branch(&self, owner: &str, repo: &str) -> DepotResult<String> {
        let repo_info = self.get_repo(owner, repo).await?;
        Ok(repo_info.default_branch)
    }

    /// Get file content from a repository
    pub async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> DepotResult<String> {
        let url = format!(
            "{}/repos/{}/{}/contents/{}?ref={}",
            self.api_url, owner, repo, path, ref_
        );

        let response = self.api_request(&url).await?;

        // GitHub returns base64-encoded content
        #[derive(serde::Deserialize)]
        struct ContentResponse {
            content: String,
            encoding: String,
        }

        let content_resp: ContentResponse = response
            .json()
            .await
            .map_err(|e| DepotError::Package(format!("Failed to parse content response: {}", e)))?;

        if content_resp.encoding != "base64" {
            return Err(DepotError::Package(format!(
                "Unexpected encoding: {}",
                content_resp.encoding
            )));
        }

        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(content_resp.content.replace('\n', ""))
            .map_err(|e| DepotError::Package(format!("Failed to decode base64 content: {}", e)))?;

        String::from_utf8(decoded)
            .map_err(|e| DepotError::Package(format!("Invalid UTF-8 in file content: {}", e)))
    }

    /// Download a tarball for a specific ref
    pub async fn download_tarball(
        &self,
        owner: &str,
        repo: &str,
        ref_: &str,
    ) -> DepotResult<PathBuf> {
        let url = format!("{}/repos/{}/{}/tarball/{}", self.api_url, owner, repo, ref_);

        // Check cache first
        let cache_path = self.cache.source_path(&url);
        if self.cache.exists(&cache_path) {
            return Ok(cache_path);
        }

        // Download
        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| DepotError::Package(format!("Failed to download tarball: {}", e)))?;

        if !response.status().is_success() {
            return Err(DepotError::Package(format!(
                "Failed to download tarball: HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| DepotError::Package(format!("Failed to read tarball: {}", e)))?;
        let bytes_vec = bytes.to_vec();

        // Write to cache
        self.cache.write(&cache_path, &bytes_vec)?;

        Ok(cache_path)
    }

    /// Resolve a version using the fallback chain
    pub async fn resolve_version(
        &self,
        owner: &str,
        repo: &str,
        version_spec: Option<&str>,
        fallback_chain: &[String],
    ) -> DepotResult<ResolvedVersion> {
        // If a specific version is requested, try to find it
        if let Some(version) = version_spec {
            // Try as release tag
            if let Ok(release) = self.get_releases(owner, repo).await {
                if let Some(r) = release.iter().find(|r| r.tag_name == version) {
                    return Ok(ResolvedVersion {
                        ref_type: RefType::Release,
                        ref_value: r.tag_name.clone(),
                        commit_sha: String::new(), // Would need additional API call
                        tarball_url: r.tarball_url.clone(),
                    });
                }
            }

            // Try as tag
            if let Ok(tags) = self.get_tags(owner, repo).await {
                if let Some(tag) = tags.iter().find(|t| t.name == version) {
                    return Ok(ResolvedVersion {
                        ref_type: RefType::Tag,
                        ref_value: tag.name.clone(),
                        commit_sha: tag.commit.sha.clone(),
                        tarball_url: tag.tarball_url.clone(),
                    });
                }
            }

            // Try as branch
            let default_branch = self.get_default_branch(owner, repo).await?;
            if version == default_branch {
                return Ok(ResolvedVersion {
                    ref_type: RefType::Branch,
                    ref_value: version.to_string(),
                    commit_sha: String::new(), // Would need additional API call
                    tarball_url: format!(
                        "{}/repos/{}/{}/tarball/{}",
                        self.api_url, owner, repo, version
                    ),
                });
            }

            return Err(DepotError::Package(format!(
                "Version {} not found for {}/{}",
                version, owner, repo
            )));
        }

        // No specific version - use fallback chain
        for strategy in fallback_chain {
            match strategy.as_str() {
                "release" => {
                    if let Ok(release) = self.get_latest_release(owner, repo).await {
                        if !release.draft && !release.prerelease {
                            return Ok(ResolvedVersion {
                                ref_type: RefType::Release,
                                ref_value: release.tag_name.clone(),
                                commit_sha: String::new(),
                                tarball_url: release.tarball_url.clone(),
                            });
                        }
                    }
                }
                "tag" => {
                    if let Ok(tags) = self.get_tags(owner, repo).await {
                        if let Some(tag) = tags.first() {
                            return Ok(ResolvedVersion {
                                ref_type: RefType::Tag,
                                ref_value: tag.name.clone(),
                                commit_sha: tag.commit.sha.clone(),
                                tarball_url: tag.tarball_url.clone(),
                            });
                        }
                    }
                }
                "branch" => {
                    let default_branch = self.get_default_branch(owner, repo).await?;
                    return Ok(ResolvedVersion {
                        ref_type: RefType::Branch,
                        ref_value: default_branch.clone(),
                        commit_sha: String::new(),
                        tarball_url: format!(
                            "{}/repos/{}/{}/tarball/{}",
                            self.api_url, owner, repo, default_branch
                        ),
                    });
                }
                _ => continue,
            }
        }

        Err(DepotError::Package(format!(
            "Could not resolve version for {}/{}",
            owner, repo
        )))
    }

    /// Make an API request and handle rate limiting
    async fn api_request(&self, url: &str) -> DepotResult<reqwest::Response> {
        // Check rate limit
        self.check_rate_limit().await?;

        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| DepotError::Package(format!("GitHub API request failed: {}", e)))?;

        // Update rate limit from headers
        self.update_rate_limit(&response).await;

        if !response.status().is_success() {
            return Err(DepotError::Package(format!(
                "GitHub API error: HTTP {}",
                response.status()
            )));
        }

        Ok(response)
    }

    /// Make an API GET request and parse JSON response
    async fn api_get<T: serde::de::DeserializeOwned>(&self, url: &str) -> DepotResult<T> {
        let response = self.api_request(url).await?;

        response
            .json()
            .await
            .map_err(|e| DepotError::Package(format!("Failed to parse GitHub API response: {}", e)))
    }

    /// Check if we're within rate limits
    async fn check_rate_limit(&self) -> DepotResult<()> {
        let remaining = *self.rate_limiter.remaining.lock().await;
        let reset_time = *self.rate_limiter.reset_time.lock().await;

        if remaining == 0 {
            let now = SystemTime::now();
            if now < reset_time {
                let wait_duration = reset_time.duration_since(now).unwrap_or(Duration::ZERO);
                return Err(DepotError::Package(format!(
                    "GitHub API rate limit exceeded. Reset in {} seconds. {}",
                    wait_duration.as_secs(),
                    if self.token.is_none() {
                        "Consider setting GITHUB_TOKEN to increase rate limit to 5000/hour."
                    } else {
                        ""
                    }
                )));
            }
        }

        Ok(())
    }

    /// Update rate limit from response headers
    async fn update_rate_limit(&self, response: &reqwest::Response) {
        if let Some(remaining) = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        {
            *self.rate_limiter.remaining.lock().await = remaining;
        }

        if let Some(reset) = response
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        {
            *self.rate_limiter.reset_time.lock().await =
                SystemTime::UNIX_EPOCH + Duration::from_secs(reset);
        }
    }
}

// Implement GitHubProvider trait
#[async_trait]
impl GitHubProvider for GitHubClient {
    async fn get_releases(&self, owner: &str, repo: &str) -> DepotResult<Vec<GitHubRelease>> {
        Self::get_releases(self, owner, repo).await
    }

    async fn get_latest_release(&self, owner: &str, repo: &str) -> DepotResult<GitHubRelease> {
        Self::get_latest_release(self, owner, repo).await
    }

    async fn get_tags(&self, owner: &str, repo: &str) -> DepotResult<Vec<GitHubTag>> {
        Self::get_tags(self, owner, repo).await
    }

    async fn get_default_branch(&self, owner: &str, repo: &str) -> DepotResult<String> {
        Self::get_default_branch(self, owner, repo).await
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> DepotResult<String> {
        Self::get_file_content(self, owner, repo, path, ref_).await
    }

    async fn download_tarball(&self, owner: &str, repo: &str, ref_: &str) -> DepotResult<PathBuf> {
        Self::download_tarball(self, owner, repo, ref_).await
    }

    async fn resolve_version(
        &self,
        owner: &str,
        repo: &str,
        version_spec: Option<&str>,
        fallback_chain: &[String],
    ) -> DepotResult<ResolvedVersion> {
        Self::resolve_version(self, owner, repo, version_spec, fallback_chain).await
    }
}
