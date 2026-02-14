//! GitHub API type definitions

use serde::{Deserialize, Serialize};

/// Type of Git reference
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefType {
    Release,
    Tag,
    Branch,
    Commit,
}

impl std::fmt::Display for RefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefType::Release => write!(f, "release"),
            RefType::Tag => write!(f, "tag"),
            RefType::Branch => write!(f, "branch"),
            RefType::Commit => write!(f, "commit"),
        }
    }
}

/// GitHub release information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub tarball_url: String,
    pub zipball_url: String,
    pub assets: Vec<ReleaseAsset>,
    pub body: Option<String>,
    pub published_at: Option<String>,
}

/// GitHub release asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub content_type: String,
}

/// GitHub tag information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubTag {
    pub name: String,
    pub commit: TagCommit,
    pub tarball_url: String,
    pub zipball_url: String,
}

/// Commit information in a tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCommit {
    pub sha: String,
    pub url: String,
}

/// GitHub repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub name: String,
    pub full_name: String,
    pub default_branch: String,
    pub description: Option<String>,
}

/// Resolved version information
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub ref_type: RefType,
    pub ref_value: String,
    pub commit_sha: String,
    pub tarball_url: String,
}

/// GitHub API rate limit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub limit: u64,
    pub remaining: u64,
    pub reset: u64,
}
