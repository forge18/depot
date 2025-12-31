use crate::core::{LpmError, LpmResult};

/// Parse and validate Lua version strings
pub fn parse_version(version: &str) -> LpmResult<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.len() < 2 {
        return Err(LpmError::Package(format!(
            "Invalid version format: {}. Expected format: X.Y.Z",
            version
        )));
    }

    let major = parts[0]
        .parse::<u32>()
        .map_err(|_| LpmError::Package(format!("Invalid major version: {}", parts[0])))?;

    let minor = parts[1]
        .parse::<u32>()
        .map_err(|_| LpmError::Package(format!("Invalid minor version: {}", parts[1])))?;

    let patch = if parts.len() > 2 {
        parts[2]
            .parse::<u32>()
            .map_err(|_| LpmError::Package(format!("Invalid patch version: {}", parts[2])))?
    } else {
        0
    };

    Ok((major, minor, patch))
}

/// Extract version code from version string (e.g., "5.4.8" -> "54")
pub fn version_code(version: &str) -> LpmResult<String> {
    let (major, minor, _) = parse_version(version)?;
    Ok(format!("{}{}", major, minor))
}

/// Compare two version strings
pub fn compare_versions(a: &str, b: &str) -> LpmResult<std::cmp::Ordering> {
    let a_parts = parse_version(a)?;
    let b_parts = parse_version(b)?;
    Ok(a_parts.cmp(&b_parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("5.4.8").unwrap(), (5, 4, 8));
        assert_eq!(parse_version("5.3.6").unwrap(), (5, 3, 6));
        assert_eq!(parse_version("5.1.5").unwrap(), (5, 1, 5));
    }

    #[test]
    fn test_version_code() {
        assert_eq!(version_code("5.4.8").unwrap(), "54");
        assert_eq!(version_code("5.3.6").unwrap(), "53");
        assert_eq!(version_code("5.1.5").unwrap(), "51");
    }

    #[test]
    fn test_compare_versions() {
        assert!(compare_versions("5.4.8", "5.3.6").unwrap().is_gt());
        assert!(compare_versions("5.3.6", "5.4.8").unwrap().is_lt());
        assert!(compare_versions("5.4.8", "5.4.8").unwrap().is_eq());
    }

    #[test]
    fn test_parse_version_two_parts() {
        // Two-part version should default patch to 0
        assert_eq!(parse_version("5.4").unwrap(), (5, 4, 0));
    }

    #[test]
    fn test_parse_version_invalid_single_part() {
        let result = parse_version("5");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid version format"));
    }

    #[test]
    fn test_parse_version_invalid_major() {
        let result = parse_version("abc.4.8");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid major version"));
    }

    #[test]
    fn test_parse_version_invalid_minor() {
        let result = parse_version("5.xyz.8");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid minor version"));
    }

    #[test]
    fn test_parse_version_invalid_patch() {
        let result = parse_version("5.4.abc");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid patch version"));
    }

    #[test]
    fn test_version_code_two_parts() {
        assert_eq!(version_code("5.4").unwrap(), "54");
    }

    #[test]
    fn test_compare_versions_patch_difference() {
        assert!(compare_versions("5.4.8", "5.4.7").unwrap().is_gt());
        assert!(compare_versions("5.4.7", "5.4.8").unwrap().is_lt());
    }

    #[test]
    fn test_compare_versions_major_difference() {
        assert!(compare_versions("6.0.0", "5.4.8").unwrap().is_gt());
    }
}
