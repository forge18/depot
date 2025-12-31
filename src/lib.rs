//! LPM (Local Package Manager) for Lua
//!
//! This crate provides the main LPM library, re-exporting core functionality
//! from `lpm-core` and organizing additional modules for package management,
//! LuaRocks integration, and related features.

pub use lpm_core::package::manifest::PackageManifest;
pub use lpm_core::path_setup::{LuaRunner, PathSetup, RunOptions};
pub use lpm_core::{format_error_with_help, CredentialStore, ErrorHelp, LpmError, LpmResult};

/// Core module re-exported for backward compatibility.
pub mod core {
    pub use lpm_core::core::*;
    pub use lpm_core::*;

    /// Path module re-exported from lpm-core.
    pub mod path {
        pub use lpm_core::core::path::*;
    }

    /// Path setup for LPM binary (not Lua paths).
    pub mod path_setup;
}

/// Configuration management.
pub mod config;

/// Package caching.
pub mod cache;

/// Package management (install, update, remove).
pub mod package;

/// LuaRocks integration.
pub mod luarocks;

/// Dependency resolution.
pub mod resolver;

/// Path setup and Lua runner (re-exported from lpm-core).
pub mod path_setup {
    pub use lpm_core::path_setup::*;
}

/// Rust extension building.
pub mod build;

/// Workspace support.
pub mod workspace;

/// Security and auditing.
pub mod security;

/// Lua version support.
pub mod lua_version;

/// Lua version manager.
pub mod lua_manager;

/// Publishing packages.
pub mod publish;

/// Dependency injection infrastructure.
pub mod di;
