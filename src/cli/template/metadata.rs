use depot_core::DepotResult;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Template metadata stored in template.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    pub name: String,
    pub description: String,
    pub author: Option<String>,
    pub version: Option<String>,
    pub variables: Vec<TemplateVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub description: Option<String>,
    pub default: Option<String>,
    pub required: bool,
}

impl TemplateMetadata {
    pub fn load(template_dir: &Path) -> DepotResult<Self> {
        let metadata_path = template_dir.join("template.yaml");
        if !metadata_path.exists() {
            return Err(depot_core::DepotError::Config(format!(
                "Template metadata not found: {}",
                metadata_path.display()
            )));
        }

        let content = std::fs::read_to_string(&metadata_path)?;
        let metadata: TemplateMetadata = serde_yaml::from_str(&content)?;
        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_template_metadata_load() {
        let temp = TempDir::new().unwrap();
        let template_dir = temp.path();

        let metadata_content = r#"
name: test-template
description: A test template
author: Test Author
version: 1.0.0
variables:
  - name: project_name
    description: Project name
    required: true
  - name: version
    description: Project version
    default: "1.0.0"
    required: false
"#;
        fs::write(template_dir.join("template.yaml"), metadata_content).unwrap();

        let metadata = TemplateMetadata::load(template_dir).unwrap();
        assert_eq!(metadata.name, "test-template");
        assert_eq!(metadata.description, "A test template");
        assert_eq!(metadata.author, Some("Test Author".to_string()));
        assert_eq!(metadata.variables.len(), 2);
        assert_eq!(metadata.variables[0].name, "project_name");
        assert!(metadata.variables[0].required);
        assert!(!metadata.variables[1].required);
    }

    #[test]
    fn test_template_metadata_load_missing_file() {
        let temp = TempDir::new().unwrap();
        let template_dir = temp.path();

        let result = TemplateMetadata::load(template_dir);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Template metadata not found"));
    }

    #[test]
    fn test_template_variable_default() {
        let var = TemplateVariable {
            name: "test_var".to_string(),
            description: Some("Test variable".to_string()),
            default: Some("default_value".to_string()),
            required: false,
        };
        assert_eq!(var.name, "test_var");
        assert_eq!(var.default, Some("default_value".to_string()));
    }
}
