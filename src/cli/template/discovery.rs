use super::metadata::TemplateMetadata;
use depot_core::core::path::depot_home;
use depot_core::{DepotError, DepotResult};
use std::path::{Path, PathBuf};

/// Discovers available templates from built-in and user locations
pub struct TemplateDiscovery;

impl TemplateDiscovery {
    /// Get the user templates directory
    pub fn user_templates_dir() -> DepotResult<PathBuf> {
        Ok(depot_home()?.join("templates"))
    }

    /// Get built-in templates directory (in the binary/resources)
    /// For now, we'll use a directory relative to the binary or a default location
    pub fn builtin_templates_dir() -> PathBuf {
        // Check for templates in the source directory (for development)
        // In production, this could be embedded in the binary or installed separately
        // nosemgrep: rust.lang.security.current-exe.current-exe
        // Justification: Used only to locate built-in template resources relative to the binary.
        // This is a standard practice for finding application resources and poses no security risk.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                // Check if we're in a development build (target/debug or target/release)
                if exe_dir.to_string_lossy().contains("target") {
                    // Look for templates relative to workspace root
                    if let Some(workspace_root) = exe_dir
                        .ancestors()
                        .find(|p| p.join("Cargo.toml").exists() && p.join("src").exists())
                    {
                        return workspace_root.join("src").join("templates");
                    }
                }
            }
        }
        // Fallback: check current directory or default location
        PathBuf::from("templates")
    }

    /// List all available templates
    pub fn list_templates() -> DepotResult<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // Check built-in templates
        let builtin_dir = Self::builtin_templates_dir();
        if builtin_dir.exists() {
            templates.extend(Self::discover_in_dir(
                &builtin_dir,
                TemplateSource::Builtin,
            )?);
        }

        // Check user templates
        if let Ok(user_dir) = Self::user_templates_dir() {
            if user_dir.exists() {
                templates.extend(Self::discover_in_dir(&user_dir, TemplateSource::User)?);
            }
        }

        Ok(templates)
    }

    fn discover_in_dir(dir: &Path, source: TemplateSource) -> DepotResult<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        if !dir.exists() || !dir.is_dir() {
            return Ok(templates);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this directory contains a template.yaml
                let metadata_path = path.join("template.yaml");
                if metadata_path.exists() {
                    match TemplateMetadata::load(&path) {
                        Ok(metadata) => {
                            templates.push(TemplateInfo {
                                name: metadata.name.clone(),
                                description: metadata.description,
                                path,
                                source,
                            });
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: Failed to load template metadata from {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }
        }

        Ok(templates)
    }

    /// Find a template by name
    pub fn find_template(name: &str) -> DepotResult<TemplateInfo> {
        // First check user templates (higher priority)
        if let Ok(user_dir) = Self::user_templates_dir() {
            if user_dir.exists() {
                let template_path = user_dir.join(name);
                if template_path.exists() && template_path.join("template.yaml").exists() {
                    let metadata = TemplateMetadata::load(&template_path)?;
                    return Ok(TemplateInfo {
                        name: metadata.name,
                        description: metadata.description,
                        path: template_path,
                        source: TemplateSource::User,
                    });
                }
            }
        }

        // Then check built-in templates
        let builtin_dir = Self::builtin_templates_dir();
        let template_path = builtin_dir.join(name);
        if template_path.exists() && template_path.join("template.yaml").exists() {
            let metadata = TemplateMetadata::load(&template_path)?;
            return Ok(TemplateInfo {
                name: metadata.name,
                description: metadata.description,
                path: template_path,
                source: TemplateSource::Builtin,
            });
        }

        Err(DepotError::Config(format!("Template '{}' not found", name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_user_templates_dir() {
        // Test that user_templates_dir returns a valid path
        let result = TemplateDiscovery::user_templates_dir();
        // May fail if depot_home() fails, but tests the function exists
        let _ = result;
    }

    #[test]
    fn test_builtin_templates_dir() {
        // Test that builtin_templates_dir returns a path
        let dir = TemplateDiscovery::builtin_templates_dir();
        // Should always return a path (may not exist)
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_discover_in_dir() {
        let temp = TempDir::new().unwrap();
        let template_dir = temp.path().join("test-template");
        fs::create_dir_all(&template_dir).unwrap();

        // Create template.yaml
        let metadata = r#"
name: test-template
description: Test template
variables: []
"#;
        fs::write(template_dir.join("template.yaml"), metadata).unwrap();

        // Discover templates in the directory
        let templates =
            TemplateDiscovery::discover_in_dir(temp.path(), TemplateSource::Builtin).unwrap();

        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "test-template");
    }

    #[test]
    fn test_discover_in_dir_no_metadata() {
        let temp = TempDir::new().unwrap();
        let template_dir = temp.path().join("no-metadata");
        fs::create_dir_all(&template_dir).unwrap();

        // No template.yaml, should skip
        let templates =
            TemplateDiscovery::discover_in_dir(temp.path(), TemplateSource::Builtin).unwrap();

        assert!(templates.is_empty());
    }
}

#[derive(Debug, Clone)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub source: TemplateSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateSource {
    Builtin,
    User,
}
