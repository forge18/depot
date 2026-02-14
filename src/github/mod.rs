//! GitHub integration for package management
//!
//! This module provides functionality for interacting with GitHub to:
//! - Fetch releases, tags, and branches
//! - Download source tarballs
//! - Retrieve file contents from repositories
//! - Resolve package versions using a fallback chain

pub mod client;
pub mod types;

pub use client::GitHubClient;
pub use types::{GitHubRelease, GitHubTag, RefType, ResolvedVersion};
