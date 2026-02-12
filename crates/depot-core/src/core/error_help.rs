use crate::core::DepotError;

/// Provides helpful suggestions for common errors
pub trait ErrorHelp {
    fn help(&self) -> Option<String>;
}

impl ErrorHelp for DepotError {
    fn help(&self) -> Option<String> {
        match self {
            DepotError::Package(msg) => {
                if msg.contains("package.yaml not found") {
                    Some(
                        "💡 Suggestion: Run 'lpm init' to create a new project, or navigate to a directory with package.yaml"
                            .to_string(),
                    )
                } else if msg.contains("not found in manifest") {
                    Some(
                        "💡 Suggestion: Check the package name spelling, or verify the package exists on LuaRocks"
                            .to_string(),
                    )
                } else if msg.contains("lua_modules directory not found") {
                    Some(
                        "💡 Suggestion: Run 'lpm install' to install dependencies first"
                            .to_string(),
                    )
                } else if msg.contains("Circular dependencies") {
                    Some(
                        "💡 Suggestion: Review your dependencies and remove circular references"
                            .to_string(),
                    )
                } else if msg.contains("Version conflict") {
                    Some(
                        "💡 Suggestion: Update package versions to resolve conflicts, or use 'lpm update' to find compatible versions"
                            .to_string(),
                    )
                } else {
                    None
                }
            }
            DepotError::Version(msg) => {
                if msg.contains("Invalid version format") {
                    Some(
                        "💡 Suggestion: Use SemVer format (e.g., '1.2.3', '^1.2.3', '~1.2.3', '>=1.2.3')"
                            .to_string(),
                    )
                } else if msg.contains("no version satisfies") {
                    Some(
                        "💡 Suggestion: Try a different version constraint, or check available versions with 'lpm list'"
                            .to_string(),
                    )
                } else {
                    None
                }
            }
            DepotError::Path(msg) => {
                if msg.contains("Could not find package.yaml") {
                    Some(
                        "💡 Suggestion: Run 'lpm init' to create a new project, or navigate to a directory with package.yaml"
                            .to_string(),
                    )
                } else if msg.contains("Could not determine") {
                    Some(
                        "💡 Suggestion: Check your system environment variables (HOME, APPDATA, etc.)"
                            .to_string(),
                    )
                } else {
                    None
                }
            }
            DepotError::Yaml(e) => {
                Some(format!(
                    "💡 Suggestion: Check your YAML syntax. Common issues:\n  - Missing colons after keys\n  - Incorrect indentation\n  - Unclosed quotes\n  - Invalid characters\n\nError details: {}",
                    e
                ))
            }
            DepotError::Http(e) => {
                if e.is_timeout() {
                    Some(
                        "💡 Suggestion: Check your internet connection, or try again later"
                            .to_string(),
                    )
                } else if e.is_connect() {
                    Some(
                        "💡 Suggestion: Check your internet connection and firewall settings"
                            .to_string(),
                    )
                } else {
                    Some(
                        "💡 Suggestion: Check your internet connection, or verify the LuaRocks server is accessible"
                            .to_string(),
                    )
                }
            }
            DepotError::Io(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    Some(
                        "💡 Suggestion: Check file permissions, or try running with appropriate permissions"
                            .to_string(),
                    )
                } else if e.kind() == std::io::ErrorKind::NotFound {
                    Some(
                        "💡 Suggestion: The file or directory may not exist. Check the path and try again"
                            .to_string(),
                    )
                } else {
                    None
                }
            }
            DepotError::LuaRocks(msg) => {
                if msg.contains("Failed to fetch") {
                    Some(
                        "💡 Suggestion: Check your internet connection, or verify the LuaRocks server is accessible"
                            .to_string(),
                    )
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Format an error with helpful suggestions
pub fn format_error_with_help(error: &DepotError) -> String {
    let mut output = format!("❌ Error: {}", error);

    if let Some(help) = error.help() {
        output.push_str("\n\n");
        output.push_str(&help);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_help_package_not_found() {
        let error = DepotError::Package("package.yaml not found in /path".to_string());
        assert!(error.help().is_some());
        assert!(error.help().unwrap().contains("lpm init"));
    }

    #[test]
    fn test_error_help_version_invalid() {
        let error = DepotError::Version("Invalid version format: xyz".to_string());
        assert!(error.help().is_some());
        assert!(error.help().unwrap().contains("SemVer"));
    }
}
