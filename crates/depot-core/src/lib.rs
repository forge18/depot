// Core functionality
pub mod core;

// Path setup and Lua runner
pub mod path_setup;

// Package manifest
pub mod package;

// Re-export commonly used types
pub use core::{format_error_with_help, CredentialStore, DepotError, DepotResult, ErrorHelp};
pub use package::manifest::PackageManifest;
pub use path_setup::{LuaRunner, PathSetup, RunOptions};
