use crate::core::{DepotError, DepotResult};

/// Supported cross-compilation targets
pub const SUPPORTED_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
];

/// Represents a build target
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub triple: String,
}

impl Target {
    pub fn new(triple: &str) -> DepotResult<Self> {
        if !SUPPORTED_TARGETS.contains(&triple) {
            return Err(DepotError::Package(format!(
                "Unsupported target '{}'. Supported targets: {}",
                triple,
                SUPPORTED_TARGETS.join(", ")
            )));
        }
        Ok(Self {
            triple: triple.to_string(),
        })
    }

    /// Get the default target for the current platform
    pub fn default_target() -> Self {
        #[cfg(target_os = "linux")]
        {
            #[cfg(target_arch = "x86_64")]
            return Self {
                triple: "x86_64-unknown-linux-gnu".to_string(),
            };
            #[cfg(target_arch = "aarch64")]
            return Self {
                triple: "aarch64-unknown-linux-gnu".to_string(),
            };
        }
        #[cfg(target_os = "macos")]
        {
            #[cfg(target_arch = "x86_64")]
            return Self {
                triple: "x86_64-apple-darwin".to_string(),
            };
            #[cfg(target_arch = "aarch64")]
            return Self {
                triple: "aarch64-apple-darwin".to_string(),
            };
        }
        #[cfg(target_os = "windows")]
        {
            Self {
                triple: "x86_64-pc-windows-gnu".to_string(),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Fallback
            Self {
                triple: "x86_64-unknown-linux-gnu".to_string(),
            }
        }
    }

    /// Get the file extension for native modules on this target
    pub fn module_extension(&self) -> &'static str {
        if self.triple.contains("windows") {
            ".dll"
        } else if self.triple.contains("darwin") {
            ".dylib"
        } else {
            ".so"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_validation() {
        assert!(Target::new("x86_64-unknown-linux-gnu").is_ok());
        assert!(Target::new("invalid-target").is_err());
    }

    #[test]
    fn test_module_extension() {
        let linux = Target::new("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(linux.module_extension(), ".so");

        let windows = Target::new("x86_64-pc-windows-gnu").unwrap();
        assert_eq!(windows.module_extension(), ".dll");

        let macos = Target::new("x86_64-apple-darwin").unwrap();
        assert_eq!(macos.module_extension(), ".dylib");
    }

    #[test]
    fn test_all_supported_targets() {
        for target in SUPPORTED_TARGETS {
            let result = Target::new(target);
            assert!(result.is_ok(), "Should support target: {}", target);
        }
    }

    #[test]
    fn test_target_clone_and_eq() {
        let target1 = Target::new("x86_64-unknown-linux-gnu").unwrap();
        let target2 = target1.clone();
        assert_eq!(target1, target2);
    }

    #[test]
    fn test_target_triple_stored_correctly() {
        let target = Target::new("aarch64-unknown-linux-gnu").unwrap();
        assert_eq!(target.triple, "aarch64-unknown-linux-gnu");
    }

    #[test]
    fn test_default_target() {
        let target = Target::default_target();
        assert!(!target.triple.is_empty());
        // Verify it's one of the supported targets
        assert!(SUPPORTED_TARGETS.contains(&target.triple.as_str()));
    }

    #[test]
    fn test_linux_musl_target() {
        let target = Target::new("x86_64-unknown-linux-musl").unwrap();
        assert_eq!(target.module_extension(), ".so");
    }

    #[test]
    fn test_aarch64_darwin_target() {
        let target = Target::new("aarch64-apple-darwin").unwrap();
        assert_eq!(target.module_extension(), ".dylib");
    }

    #[test]
    fn test_invalid_target_error_message() {
        let result = Target::new("invalid-triple");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported target"));
        assert!(err.contains("invalid-triple"));
    }

    #[test]
    fn test_supported_targets_list_not_empty() {
        assert!(!SUPPORTED_TARGETS.is_empty());
        assert!(SUPPORTED_TARGETS.len() >= 6);
    }

    #[test]
    fn test_target_debug_impl() {
        let target = Target::new("x86_64-unknown-linux-gnu").unwrap();
        let debug_str = format!("{:?}", target);
        assert!(debug_str.contains("x86_64-unknown-linux-gnu"));
    }
}
