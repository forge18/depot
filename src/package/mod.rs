pub mod checksum;
pub mod conflict_checker;
pub mod converter;
pub mod downloader;
pub mod extractor;
pub mod installer;
pub mod interactive;
pub mod lockfile;
pub mod lockfile_builder;
// manifest moved to lpm-core, re-export for backward compatibility
pub mod manifest {
    pub use depot_core::package::manifest::*;
}
pub mod packager;
pub mod rollback;
pub mod update_diff;
pub mod validator;
pub mod verifier;

pub use checksum::ChecksumRecorder;
pub use conflict_checker::ConflictChecker;
pub use converter::convert_rockspec_to_manifest;
pub use extractor::PackageExtractor;
pub use installer::PackageInstaller;
pub use lockfile::Lockfile;
pub use lockfile_builder::LockfileBuilder;
pub use manifest::PackageManifest;
pub use rollback::{with_rollback, RollbackManager};
pub use validator::ManifestValidator;
pub use verifier::{PackageVerifier, VerificationResult};
