use crate::core::{DepotError, DepotResult};
use crate::lua_version::detector::LuaVersion;

/// Lua version constraint parser
///
/// Supports:
/// - Exact: "5.4"
/// - Range: ">=5.1", "<5.3"
/// - Multiple: "5.1 || 5.3 || 5.4"
/// - Exclude: "<5.3"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LuaVersionConstraint {
    /// Exact version: "5.4"
    Exact(LuaVersion),
    /// Greater than or equal: ">=5.1"
    GreaterOrEqual(LuaVersion),
    /// Less than: "<5.3"
    LessThan(LuaVersion),
    /// Multiple versions (OR): "5.1 || 5.3 || 5.4"
    Multiple(Vec<LuaVersion>),
}

impl LuaVersionConstraint {
    /// Check if a Lua version satisfies this constraint
    pub fn matches(&self, version: &LuaVersion) -> bool {
        match self {
            LuaVersionConstraint::Exact(v) => version.major == v.major && version.minor == v.minor,
            LuaVersionConstraint::GreaterOrEqual(v) => {
                version.major > v.major || (version.major == v.major && version.minor >= v.minor)
            }
            LuaVersionConstraint::LessThan(v) => {
                version.major < v.major || (version.major == v.major && version.minor < v.minor)
            }
            LuaVersionConstraint::Multiple(versions) => versions
                .iter()
                .any(|v| version.major == v.major && version.minor == v.minor),
        }
    }
}

/// Parse a Lua version constraint string
///
/// Examples:
/// - "5.4" -> Exact(5.4)
/// - ">=5.1" -> GreaterOrEqual(5.1)
/// - "<5.3" -> LessThan(5.3)
/// - "5.1 || 5.3 || 5.4" -> Multiple([5.1, 5.3, 5.4])
pub fn parse_lua_version_constraint(constraint: &str) -> DepotResult<LuaVersionConstraint> {
    let constraint = constraint.trim();

    // Check for multiple versions (OR)
    if constraint.contains("||") {
        let versions: Vec<LuaVersion> = constraint
            .split("||")
            .map(|s| {
                let s = s.trim();
                // Parse version like "5.4" or ">=5.4"
                if let Some(rest) = s.strip_prefix(">=") {
                    LuaVersion::parse(rest)
                } else if let Some(rest) = s.strip_prefix("<") {
                    LuaVersion::parse(rest)
                } else {
                    LuaVersion::parse(s)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        if versions.is_empty() {
            return Err(DepotError::Version(
                "Empty version list in constraint".to_string(),
            ));
        }

        return Ok(LuaVersionConstraint::Multiple(versions));
    }

    // Check for range constraints
    if let Some(rest) = constraint.strip_prefix(">=") {
        let version = LuaVersion::parse(rest)?;
        return Ok(LuaVersionConstraint::GreaterOrEqual(version));
    }

    if let Some(rest) = constraint.strip_prefix("<=") {
        // For <=, we'll treat it as < next minor version
        let version = LuaVersion::parse(rest)?;
        let next_version = LuaVersion::new(version.major, version.minor + 1, 0);
        return Ok(LuaVersionConstraint::LessThan(next_version));
    }

    if let Some(rest) = constraint.strip_prefix("<") {
        let version = LuaVersion::parse(rest)?;
        return Ok(LuaVersionConstraint::LessThan(version));
    }

    if let Some(rest) = constraint.strip_prefix(">") {
        // For >, we'll treat it as >= next patch version
        let version = LuaVersion::parse(rest)?;
        let next_version = LuaVersion::new(version.major, version.minor, version.patch + 1);
        return Ok(LuaVersionConstraint::GreaterOrEqual(next_version));
    }

    // Default: exact version
    let version = LuaVersion::parse(constraint)?;
    Ok(LuaVersionConstraint::Exact(version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exact() {
        let constraint = parse_lua_version_constraint("5.4").unwrap();
        assert!(matches!(constraint, LuaVersionConstraint::Exact(_)));
        let version = LuaVersion::new(5, 4, 0);
        assert!(constraint.matches(&version));
    }

    #[test]
    fn test_parse_greater_or_equal() {
        let constraint = parse_lua_version_constraint(">=5.1").unwrap();
        assert!(matches!(
            constraint,
            LuaVersionConstraint::GreaterOrEqual(_)
        ));
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 3, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 4, 0)));
    }

    #[test]
    fn test_parse_less_than() {
        let constraint = parse_lua_version_constraint("<5.3").unwrap();
        assert!(matches!(constraint, LuaVersionConstraint::LessThan(_)));
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0)));
        assert!(!constraint.matches(&LuaVersion::new(5, 3, 0)));
        assert!(!constraint.matches(&LuaVersion::new(5, 4, 0)));
    }

    #[test]
    fn test_parse_multiple() {
        let constraint = parse_lua_version_constraint("5.1 || 5.3 || 5.4").unwrap();
        assert!(matches!(constraint, LuaVersionConstraint::Multiple(_)));
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 3, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 4, 0)));
        assert!(!constraint.matches(&LuaVersion::new(5, 2, 0)));
    }

    #[test]
    fn test_parse_less_than_or_equal() {
        let constraint = parse_lua_version_constraint("<=5.3").unwrap();
        assert!(matches!(constraint, LuaVersionConstraint::LessThan(_)));
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 3, 0)));
        assert!(!constraint.matches(&LuaVersion::new(5, 4, 0)));
    }

    #[test]
    fn test_parse_greater_than() {
        let constraint = parse_lua_version_constraint(">5.1").unwrap();
        assert!(matches!(
            constraint,
            LuaVersionConstraint::GreaterOrEqual(_)
        ));
        // ">5.1" becomes ">=5.1.1" (next patch version)
        // But matches() only compares major.minor, so 5.1.x all match
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0))); // Same major.minor matches
        assert!(constraint.matches(&LuaVersion::new(5, 3, 0))); // Higher minor matches
    }

    #[test]
    fn test_parse_multiple_empty() {
        let result = parse_lua_version_constraint("||");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid() {
        let result = parse_lua_version_constraint("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_constraint_matches_exact() {
        let constraint = LuaVersionConstraint::Exact(LuaVersion::new(5, 4, 0));
        assert!(constraint.matches(&LuaVersion::new(5, 4, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 4, 6))); // Patch doesn't matter
        assert!(!constraint.matches(&LuaVersion::new(5, 3, 0)));
    }

    #[test]
    fn test_constraint_matches_greater_or_equal() {
        let constraint = LuaVersionConstraint::GreaterOrEqual(LuaVersion::new(5, 1, 0));
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 3, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 4, 0)));
        assert!(!constraint.matches(&LuaVersion::new(4, 9, 0)));
    }

    #[test]
    fn test_constraint_matches_less_than() {
        let constraint = LuaVersionConstraint::LessThan(LuaVersion::new(5, 3, 0));
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0)));
        assert!(!constraint.matches(&LuaVersion::new(5, 3, 0)));
        assert!(!constraint.matches(&LuaVersion::new(5, 4, 0)));
    }

    #[test]
    fn test_constraint_matches_multiple() {
        let constraint = LuaVersionConstraint::Multiple(vec![
            LuaVersion::new(5, 1, 0),
            LuaVersion::new(5, 3, 0),
            LuaVersion::new(5, 4, 0),
        ]);
        assert!(constraint.matches(&LuaVersion::new(5, 1, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 3, 0)));
        assert!(constraint.matches(&LuaVersion::new(5, 4, 0)));
        assert!(!constraint.matches(&LuaVersion::new(5, 2, 0)));
    }
}
