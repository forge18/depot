use crate::core::error::{DepotError, DepotResult};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Version constraint types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionConstraint {
    /// Exact version: "1.2.3"
    Exact(Version),
    /// Compatible version: "^1.2.3" (>=1.2.3 <2.0.0)
    Compatible(Version),
    /// Patch version: "~1.2.3" (>=1.2.3 <1.3.0)
    Patch(Version),
    /// Greater than or equal: ">=1.2.3"
    GreaterOrEqual(Version),
    /// Less than: "<2.0.0"
    LessThan(Version),
    /// Any patch version: "1.2.x"
    AnyPatch(Version),
    /// Range: >= lower AND < upper
    Range { lower: Version, upper: Version },
    /// Any of the given constraints (OR semantics)
    AnyOf(Vec<VersionConstraint>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Pre-release version (e.g., "alpha.1", "beta.2", "rc.1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<String>,
    /// Build metadata (e.g., "build.123")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_metadata: Option<String>,
}

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
            build_metadata: None,
        }
    }

    /// Create a new version with pre-release metadata
    pub fn with_prerelease(major: u64, minor: u64, patch: u64, prerelease: String) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: Some(prerelease),
            build_metadata: None,
        }
    }

    /// Parse a version string (e.g., "1.2.3", "1.2.3-alpha.1", "1.2.3+build.123", "1.2.3-rc.1+build.456")
    pub fn parse(s: &str) -> DepotResult<Self> {
        let s = s.trim();

        // Split by '+' to separate build metadata
        let (version_prerelease, build_metadata) = if let Some(pos) = s.find('+') {
            let build = s[pos + 1..].to_string();
            (&s[..pos], Some(build))
        } else {
            (s, None)
        };

        // Split by '-' to separate pre-release (but handle dash-separated versions too)
        let (version_part, prerelease) = if let Some(pos) = version_prerelease.rfind('-') {
            let potential_prerelease = &version_prerelease[pos + 1..];
            let version_before_dash = &version_prerelease[..pos];

            // If it contains non-numeric characters or dots, it's a pre-release
            if potential_prerelease.contains('.')
                || potential_prerelease.chars().any(|c| !c.is_ascii_digit())
            {
                (version_before_dash, Some(potential_prerelease.to_string()))
            } else {
                // Check if version_before_dash already has 3 parts (major.minor.patch)
                // If so, treat the numeric suffix as a prerelease per SemVer
                // Otherwise, treat "3.0-1" as "3.0.1" (dash as patch separator)
                let parts_count = version_before_dash.split('.').count();
                if parts_count >= 3 {
                    // Already has major.minor.patch, so "-1" is a prerelease
                    (version_before_dash, Some(potential_prerelease.to_string()))
                } else {
                    // Dash-separated format: "3.0-1" -> treat as patch version
                    (version_prerelease, None)
                }
            }
        } else {
            (version_prerelease, None)
        };

        // Handle dash-separated format: "3.0-1" -> "3.0.1" (when not a pre-release)
        let normalized = if prerelease.is_none() {
            version_part.replace('-', ".")
        } else {
            version_part.to_string()
        };

        let parts: Vec<&str> = normalized.split('.').collect();

        if parts.is_empty() {
            return Err(DepotError::Version(format!(
                "Invalid version format: {}",
                s
            )));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| DepotError::Version(format!("Invalid major version: {}", s)))?;
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
            build_metadata,
        })
    }

    /// Check if this version satisfies a constraint
    pub fn satisfies(&self, constraint: &VersionConstraint) -> bool {
        match constraint {
            VersionConstraint::Exact(v) => self == v,
            VersionConstraint::Compatible(v) => {
                self >= v
                    && self.major == v.major
                    && (self.major, self.minor, self.patch) < (v.major + 1, 0, 0)
            }
            VersionConstraint::Patch(v) => {
                self >= v
                    && (self.major, self.minor) == (v.major, v.minor)
                    && (self.major, self.minor, self.patch) < (v.major, v.minor + 1, 0)
            }
            VersionConstraint::GreaterOrEqual(v) => self >= v,
            VersionConstraint::LessThan(v) => self < v,
            VersionConstraint::AnyPatch(v) => self.major == v.major && self.minor == v.minor,
            VersionConstraint::Range { lower, upper } => self >= lower && self < upper,
            VersionConstraint::AnyOf(constraints) => constraints.iter().any(|c| self.satisfies(c)),
        }
    }
}

// Implement PartialEq and Eq manually to ignore build_metadata (per SemVer spec)
impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.prerelease == other.prerelease
        // build_metadata is intentionally ignored
    }
}

impl Eq for Version {}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare major, minor, patch first
        match (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)) {
            std::cmp::Ordering::Equal => {
                // Build metadata is ignored in version precedence (per SemVer spec)
                // Pre-release versions have lower precedence than normal versions
                match (&self.prerelease, &other.prerelease) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (Some(_), None) => std::cmp::Ordering::Less, // Pre-release < release
                    (None, Some(_)) => std::cmp::Ordering::Greater, // Release > pre-release
                    (Some(a), Some(b)) => {
                        // Compare pre-release identifiers
                        compare_prerelease_identifiers(a, b)
                    }
                }
            }
            other => other,
        }
    }
}

/// Compare pre-release identifiers according to SemVer spec
fn compare_prerelease_identifiers(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        // Try to parse as numbers first
        let ordering = match (a_part.parse::<u64>(), b_part.parse::<u64>()) {
            (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less, // Numeric < alphanumeric
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater, // Alphanumeric > numeric
            (Err(_), Err(_)) => a_part.cmp(b_part),      // Lexical comparison
        };

        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }

    // If all parts are equal, longer pre-release is greater
    a_parts.len().cmp(&b_parts.len())
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.prerelease {
            write!(f, "-{}", pre)?;
        }
        if let Some(ref build) = self.build_metadata {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

/// Parse a version constraint string
pub fn parse_constraint(s: &str) -> DepotResult<VersionConstraint> {
    let s = s.trim();

    if let Some(rest) = s.strip_prefix('^') {
        let version = Version::parse(rest)?;
        Ok(VersionConstraint::Compatible(version))
    } else if let Some(rest) = s.strip_prefix('~') {
        let version = Version::parse(rest)?;
        Ok(VersionConstraint::Patch(version))
    } else if let Some(rest) = s.strip_prefix(">=") {
        let version = Version::parse(rest)?;
        Ok(VersionConstraint::GreaterOrEqual(version))
    } else if let Some(rest) = s.strip_prefix('<') {
        let version = Version::parse(rest)?;
        Ok(VersionConstraint::LessThan(version))
    } else if let Some(base) = s.strip_suffix(".x") {
        let version = Version::parse(base)?;
        Ok(VersionConstraint::AnyPatch(version))
    } else {
        // Exact version
        let version = Version::parse(s)?;
        Ok(VersionConstraint::Exact(version))
    }
}

/// Parse a compound constraint string with `||` (OR) and `, ` (range) operators.
///
/// Examples: `<2.0.0`, `>=0.0.0, <2.0.0`, `>=0.0.0, <2.0.0 || >=2.5.0, <3.0.0`
pub fn parse_compound_constraint(s: &str) -> DepotResult<VersionConstraint> {
    let s = s.trim();

    // Split on " || " for AnyOf
    let or_parts: Vec<&str> = s.split(" || ").collect();
    if or_parts.len() > 1 {
        let constraints: Result<Vec<_>, _> = or_parts
            .iter()
            .map(|part| parse_compound_constraint(part))
            .collect();
        return Ok(VersionConstraint::AnyOf(constraints?));
    }

    // Split on ", " for Range (>=X, <Y)
    let and_parts: Vec<&str> = s.split(", ").collect();
    if and_parts.len() == 2 {
        if let (Some(ge), Some(lt)) = (
            and_parts[0].strip_prefix(">="),
            and_parts[1].strip_prefix("<"),
        ) {
            let lower = Version::parse(ge.trim())?;
            let upper = Version::parse(lt.trim())?;
            return Ok(VersionConstraint::Range { lower, upper });
        }
    }

    // Fall back to single constraint
    parse_constraint(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_parse_dash_separated() {
        let v = Version::parse("3.0-1").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 1);
    }

    #[test]
    fn test_version_satisfies_exact() {
        let v = Version::parse("1.2.3").unwrap();
        let constraint = VersionConstraint::Exact(Version::parse("1.2.3").unwrap());
        assert!(v.satisfies(&constraint));
    }

    #[test]
    fn test_version_satisfies_compatible() {
        let v1 = Version::parse("1.2.3").unwrap();
        let v2 = Version::parse("1.3.0").unwrap();
        let v3 = Version::parse("2.0.0").unwrap();

        let constraint = VersionConstraint::Compatible(Version::parse("1.2.0").unwrap());
        assert!(v1.satisfies(&constraint));
        assert!(v2.satisfies(&constraint));
        assert!(!v3.satisfies(&constraint));
    }

    #[test]
    fn test_parse_constraint() {
        assert!(matches!(
            parse_constraint("^1.2.3").unwrap(),
            VersionConstraint::Compatible(_)
        ));
        assert!(matches!(
            parse_constraint("~1.2.3").unwrap(),
            VersionConstraint::Patch(_)
        ));
        assert!(matches!(
            parse_constraint(">=1.2.3").unwrap(),
            VersionConstraint::GreaterOrEqual(_)
        ));
        assert!(matches!(
            parse_constraint("1.2.3").unwrap(),
            VersionConstraint::Exact(_)
        ));
    }

    #[test]
    fn test_version_with_prerelease() {
        let v = Version::parse("1.0.0-alpha.1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert_eq!(v.prerelease, Some("alpha.1".to_string()));
        assert_eq!(v.build_metadata, None);
        assert_eq!(v.to_string(), "1.0.0-alpha.1");
    }

    #[test]
    fn test_version_with_build_metadata() {
        let v = Version::parse("1.0.0+build.123").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert_eq!(v.prerelease, None);
        assert_eq!(v.build_metadata, Some("build.123".to_string()));
        assert_eq!(v.to_string(), "1.0.0+build.123");
    }

    #[test]
    fn test_version_with_prerelease_and_build() {
        let v = Version::parse("1.0.0-rc.1+build.456").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert_eq!(v.prerelease, Some("rc.1".to_string()));
        assert_eq!(v.build_metadata, Some("build.456".to_string()));
        assert_eq!(v.to_string(), "1.0.0-rc.1+build.456");
    }

    #[test]
    fn test_version_ordering_with_prerelease() {
        let v1 = Version::parse("1.0.0-alpha").unwrap();
        let v2 = Version::parse("1.0.0-alpha.1").unwrap();
        let v3 = Version::parse("1.0.0-alpha.beta").unwrap();
        let v4 = Version::parse("1.0.0-beta").unwrap();
        let v5 = Version::parse("1.0.0-beta.2").unwrap();
        let v6 = Version::parse("1.0.0-beta.11").unwrap();
        let v7 = Version::parse("1.0.0-rc.1").unwrap();
        let v8 = Version::parse("1.0.0").unwrap();

        // Per SemVer spec: 1.0.0-alpha < 1.0.0-alpha.1 < 1.0.0-alpha.beta < 1.0.0-beta
        // < 1.0.0-beta.2 < 1.0.0-beta.11 < 1.0.0-rc.1 < 1.0.0
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v4 < v5);
        assert!(v5 < v6);
        assert!(v6 < v7);
        assert!(v7 < v8);
    }

    #[test]
    fn test_version_build_metadata_ignored_in_comparison() {
        let v1 = Version::parse("1.0.0+build.1").unwrap();
        let v2 = Version::parse("1.0.0+build.2").unwrap();
        let v3 = Version::parse("1.0.0").unwrap();

        // Build metadata should be ignored in version precedence
        assert_eq!(v1, v3);
        assert_eq!(v2, v3);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_version_prerelease_numeric_vs_alphabetic() {
        let v1 = Version::parse("1.0.0-1").unwrap();
        let v2 = Version::parse("1.0.0-alpha").unwrap();

        // Numeric identifiers should be less than alphanumeric
        assert!(v1 < v2);
    }

    #[test]
    fn test_version_with_prerelease_constructor() {
        let v = Version::with_prerelease(1, 0, 0, "beta.1".to_string());
        assert_eq!(v.major, 1);
        assert_eq!(v.prerelease, Some("beta.1".to_string()));
        assert_eq!(v.to_string(), "1.0.0-beta.1");
    }

    #[test]
    fn test_version_dash_separated_format_still_works() {
        // Ensure backward compatibility with dash-separated format (e.g., "3.0-1")
        let v = Version::parse("3.0-1").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 1);
        assert_eq!(v.prerelease, None);
    }

    #[test]
    fn test_version_satisfies_with_prerelease() {
        let v_release = Version::parse("1.0.0").unwrap();
        let v_prerelease = Version::parse("1.0.0-alpha").unwrap();

        let constraint = VersionConstraint::GreaterOrEqual(Version::parse("1.0.0-alpha").unwrap());

        // Both should satisfy >=1.0.0-alpha
        assert!(v_prerelease.satisfies(&constraint));
        assert!(v_release.satisfies(&constraint));
    }

    #[test]
    fn test_version_satisfies_range() {
        let constraint = VersionConstraint::Range {
            lower: Version::new(0, 0, 0),
            upper: Version::new(2, 0, 0),
        };

        assert!(Version::new(0, 0, 0).satisfies(&constraint));
        assert!(Version::new(1, 5, 0).satisfies(&constraint));
        assert!(!Version::new(2, 0, 0).satisfies(&constraint));
        assert!(!Version::new(3, 0, 0).satisfies(&constraint));
    }

    #[test]
    fn test_version_satisfies_any_of() {
        let constraint = VersionConstraint::AnyOf(vec![
            VersionConstraint::Range {
                lower: Version::new(0, 0, 0),
                upper: Version::new(2, 0, 0),
            },
            VersionConstraint::Range {
                lower: Version::new(2, 5, 0),
                upper: Version::new(3, 0, 0),
            },
        ]);

        assert!(Version::new(1, 0, 0).satisfies(&constraint));
        assert!(Version::new(2, 7, 0).satisfies(&constraint));
        assert!(!Version::new(2, 3, 0).satisfies(&constraint));
        assert!(!Version::new(3, 0, 0).satisfies(&constraint));
    }

    #[test]
    fn test_parse_compound_constraint_simple() {
        let c = parse_compound_constraint("<2.0.0").unwrap();
        assert!(matches!(c, VersionConstraint::LessThan(_)));
    }

    #[test]
    fn test_parse_compound_constraint_range() {
        let c = parse_compound_constraint(">=1.0.0, <2.0.0").unwrap();
        assert!(matches!(c, VersionConstraint::Range { .. }));
        assert!(Version::new(1, 5, 0).satisfies(&c));
        assert!(!Version::new(0, 9, 0).satisfies(&c));
        assert!(!Version::new(2, 0, 0).satisfies(&c));
    }

    #[test]
    fn test_parse_compound_constraint_any_of() {
        let c = parse_compound_constraint(">=0.0.0, <2.0.0 || >=2.5.0, <3.0.0").unwrap();
        assert!(Version::new(1, 0, 0).satisfies(&c));
        assert!(Version::new(2, 7, 0).satisfies(&c));
        assert!(!Version::new(2, 3, 0).satisfies(&c));
        assert!(!Version::new(3, 0, 0).satisfies(&c));
    }
}
