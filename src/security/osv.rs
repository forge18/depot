use crate::core::version::{Version, VersionConstraint};
use crate::core::{DepotError, DepotResult};
use crate::security::vulnerability::{Severity, Vulnerability};
use serde::{Deserialize, Serialize};

/// Client for querying OSV (Open Source Vulnerabilities) API
pub struct OsvApi {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Serialize)]
struct OsvQuery {
    package: OsvPackage,
    version: String,
}

#[derive(Serialize)]
struct OsvPackage {
    ecosystem: String,
    name: String,
}

#[derive(Deserialize)]
struct OsvResponse {
    #[serde(default)]
    vulns: Vec<OsvVulnerability>,
}

#[derive(Deserialize)]
struct OsvVulnerability {
    id: String,
    summary: String,
    details: String,
    #[serde(default)]
    severity: Vec<OsvSeverity>,
    #[serde(default)]
    affected: Vec<OsvAffected>,
    #[serde(default)]
    references: Vec<OsvReference>,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Deserialize)]
struct OsvSeverity {
    #[serde(rename = "type")]
    severity_type: String,
    score: String,
}

#[derive(Deserialize)]
struct OsvAffected {
    #[serde(default)]
    ranges: Vec<OsvRange>,
}

#[derive(Deserialize)]
struct OsvRange {
    #[serde(rename = "type")]
    range_type: String,
    #[serde(default)]
    events: Vec<OsvEvent>,
}

#[derive(Deserialize)]
struct OsvEvent {
    #[serde(default)]
    introduced: Option<String>,
    #[serde(default)]
    fixed: Option<String>,
}

#[derive(Deserialize)]
struct OsvReference {
    url: String,
}

impl Default for OsvApi {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.osv.dev".to_string(),
        }
    }
}

impl OsvApi {
    /// Create a new OSV API client
    pub fn new() -> Self {
        Self::default()
    }

    /// Query OSV API for vulnerabilities in a package version.
    /// Returns empty vector if no vulnerabilities found or on API errors (non-fatal).
    pub async fn query_package(
        &self,
        name: &str,
        version: &str,
    ) -> DepotResult<Vec<Vulnerability>> {
        let query = OsvQuery {
            package: OsvPackage {
                ecosystem: "Lua".to_string(),
                name: name.to_string(),
            },
            version: version.to_string(),
        };

        let url = format!("{}/v1/query", self.base_url);
        let response = self
            .client
            .post(url)
            .json(&query)
            .send()
            .await
            .map_err(DepotError::Http)?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let osv_response: OsvResponse = response
            .json()
            .await
            .map_err(|e| DepotError::Package(format!("OSV parse error: {}", e)))?;

        Ok(osv_response
            .vulns
            .into_iter()
            .map(|v| {
                let severity = parse_severity(&v.severity);
                let affected_versions = parse_affected_versions(&v.affected);
                let fixed_in = extract_fixed_version(&v.affected);
                let references: Vec<String> = v.references.iter().map(|r| r.url.clone()).collect();

                // Extract CVE: use id if it's a CVE, otherwise check aliases
                let cve = if v.id.starts_with("CVE-") {
                    Some(v.id.clone())
                } else {
                    v.aliases
                        .iter()
                        .find(|a| a.starts_with("CVE-"))
                        .cloned()
                        .or_else(|| Some(v.id.clone()))
                };

                Vulnerability {
                    package: name.to_string(),
                    affected_versions,
                    severity,
                    title: v.summary,
                    description: v.details,
                    cve,
                    fixed_in,
                    references,
                }
            })
            .collect())
    }
}

/// Parse CVSS severity from OSV severity entries
fn parse_severity(severity_entries: &[OsvSeverity]) -> Severity {
    severity_entries
        .iter()
        .find(|s| s.severity_type == "CVSS_V3")
        .or_else(|| severity_entries.first())
        .and_then(|s| {
            s.score.parse::<f64>().ok().map(|score| {
                if score >= 9.0 {
                    Severity::Critical
                } else if score >= 7.0 {
                    Severity::High
                } else if score >= 4.0 {
                    Severity::Medium
                } else {
                    Severity::Low
                }
            })
        })
        .unwrap_or(Severity::Medium)
}

/// Parse OSV affected ranges into a human-readable version constraint string.
///
/// OSV events come in pairs: `{introduced: X}, {fixed: Y}` meaning
/// the vulnerability affects versions in `[X, Y)`. Multiple pairs
/// are combined with `||` (OR semantics).
fn parse_affected_versions(affected: &[OsvAffected]) -> String {
    let ranges: Vec<VersionConstraint> = affected
        .iter()
        .flat_map(|a| &a.ranges)
        .filter(|r| r.range_type == "SEMVER" || r.range_type == "ECOSYSTEM")
        .filter_map(|r| parse_osv_events(&r.events))
        .collect();

    let constraint = match ranges.len() {
        0 => return "<999.0.0".to_string(),
        1 => match ranges.into_iter().next() {
            Some(c) => c,
            None => return "<999.0.0".to_string(), // Should never happen since len == 1
        },
        _ => {
            // Flatten nested AnyOf
            let flat: Vec<VersionConstraint> = ranges
                .into_iter()
                .flat_map(|c| match c {
                    VersionConstraint::AnyOf(inner) => inner,
                    other => vec![other],
                })
                .collect();
            VersionConstraint::AnyOf(flat)
        }
    };

    format_constraint(&constraint)
}

/// Convert OSV range events into a VersionConstraint
fn parse_osv_events(events: &[OsvEvent]) -> Option<VersionConstraint> {
    let mut ranges = Vec::new();
    let mut current_introduced: Option<Version> = None;

    for event in events {
        if let Some(ref intro) = event.introduced {
            let intro_str = if intro == "0" { "0.0.0" } else { intro };
            if let Ok(v) = Version::parse(intro_str) {
                current_introduced = Some(v);
            }
        }
        if let Some(ref fixed) = event.fixed {
            if let (Some(lower), Ok(upper)) = (current_introduced.take(), Version::parse(fixed)) {
                ranges.push(VersionConstraint::Range { lower, upper });
            }
        }
    }

    // introduced without a matching fixed => all versions >= introduced
    if let Some(lower) = current_introduced {
        ranges.push(VersionConstraint::GreaterOrEqual(lower));
    }

    match ranges.len() {
        0 => None,
        1 => ranges.into_iter().next(), // Safe: len == 1
        _ => Some(VersionConstraint::AnyOf(ranges)),
    }
}

/// Extract the first fixed version from OSV affected entries
fn extract_fixed_version(affected: &[OsvAffected]) -> Option<String> {
    affected
        .iter()
        .flat_map(|a| &a.ranges)
        .flat_map(|r| &r.events)
        .find_map(|e| e.fixed.clone())
}

/// Format a VersionConstraint as a human-readable string
fn format_constraint(constraint: &VersionConstraint) -> String {
    match constraint {
        VersionConstraint::Range { lower, upper } => format!(">={}, <{}", lower, upper),
        VersionConstraint::GreaterOrEqual(v) => format!(">={}", v),
        VersionConstraint::LessThan(v) => format!("<{}", v),
        VersionConstraint::AnyOf(cs) => cs
            .iter()
            .map(format_constraint)
            .collect::<Vec<_>>()
            .join(" || "),
        VersionConstraint::Exact(v) => v.to_string(),
        VersionConstraint::Compatible(v) => format!("^{}", v),
        VersionConstraint::Patch(v) => format!("~{}", v),
        VersionConstraint::AnyPatch(v) => format!("{}.{}.x", v.major, v.minor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osv_api_new() {
        let api = OsvApi::new();
        assert_eq!(api.base_url, "https://api.osv.dev");
    }

    #[test]
    fn test_osv_api_default() {
        let api = OsvApi::default();
        assert_eq!(api.base_url, "https://api.osv.dev");
    }

    #[test]
    fn test_osv_query_serialization() {
        let query = OsvQuery {
            package: OsvPackage {
                ecosystem: "Lua".to_string(),
                name: "test-package".to_string(),
            },
            version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("test-package"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("Lua"));
    }

    #[tokio::test]
    async fn test_query_package_no_vulnerabilities() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Mock successful response with no vulnerabilities
        Mock::given(method("POST"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": []
            })))
            .mount(&mock_server)
            .await;

        let mut api = OsvApi::new();
        api.base_url = mock_server.uri();

        let result = api.query_package("test-package", "1.0.0").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_query_package_with_vulnerabilities() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": [
                    {
                        "id": "CVE-2023-12345",
                        "summary": "Test vulnerability",
                        "details": "Test details",
                        "severity": [
                            {
                                "type": "CVSS_V3",
                                "score": "9.5"
                            }
                        ],
                        "affected": [{
                            "ranges": [{
                                "type": "SEMVER",
                                "events": [
                                    {"introduced": "0"},
                                    {"fixed": "2.0.0"}
                                ]
                            }]
                        }],
                        "references": [
                            {"url": "https://example.com/advisory"}
                        ]
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let mut api = OsvApi::new();
        api.base_url = mock_server.uri();

        let result = api.query_package("test-package", "1.0.0").await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].cve, Some("CVE-2023-12345".to_string()));
        assert_eq!(result[0].severity, Severity::Critical);
        assert_eq!(result[0].fixed_in, Some("2.0.0".to_string()));
        assert_eq!(result[0].affected_versions, ">=0.0.0, <2.0.0");
        assert_eq!(result[0].references, vec!["https://example.com/advisory"]);
    }

    #[tokio::test]
    async fn test_query_package_severity_levels() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let test_cases = vec![
            (9.5, Severity::Critical),
            (8.0, Severity::High),
            (5.0, Severity::Medium),
            (2.0, Severity::Low),
        ];

        for (score, expected_severity) in test_cases {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/v1/query"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "vulns": [
                        {
                            "id": "TEST-001",
                            "summary": "Test",
                            "details": "Test",
                            "severity": [
                                {
                                    "type": "CVSS_V3",
                                    "score": score.to_string()
                                }
                            ]
                        }
                    ]
                })))
                .mount(&mock_server)
                .await;

            let mut api = OsvApi::new();
            api.base_url = mock_server.uri();

            let result = api.query_package("test", "1.0.0").await.unwrap();
            assert_eq!(result[0].severity, expected_severity, "Score: {}", score);
        }
    }

    #[tokio::test]
    async fn test_query_package_non_200_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Mock 404 response (should return empty vector, not error)
        Mock::given(method("POST"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let mut api = OsvApi::new();
        api.base_url = mock_server.uri();

        let result = api.query_package("test-package", "1.0.0").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_query_package_no_severity() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Mock response without severity (should default to Medium)
        Mock::given(method("POST"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": [
                    {
                        "id": "TEST-001",
                        "summary": "Test",
                        "details": "Test"
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let mut api = OsvApi::new();
        api.base_url = mock_server.uri();

        let result = api.query_package("test", "1.0.0").await.unwrap();
        assert_eq!(result[0].severity, Severity::Medium);
    }

    #[tokio::test]
    async fn test_query_package_with_aliases_and_references() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": [{
                    "id": "GHSA-xxxx-xxxx-xxxx",
                    "summary": "Test",
                    "details": "Test details",
                    "aliases": ["CVE-2023-99999"],
                    "affected": [{
                        "ranges": [{
                            "type": "SEMVER",
                            "events": [
                                {"introduced": "1.0.0"},
                                {"fixed": "1.5.0"}
                            ]
                        }]
                    }],
                    "references": [
                        {"url": "https://github.com/advisory/1"},
                        {"url": "https://nvd.nist.gov/vuln/detail/CVE-2023-99999"}
                    ]
                }]
            })))
            .mount(&mock_server)
            .await;

        let mut api = OsvApi::new();
        api.base_url = mock_server.uri();

        let result = api.query_package("test-pkg", "1.2.0").await.unwrap();
        assert_eq!(result.len(), 1);
        // Should extract CVE from aliases since id is a GHSA
        assert_eq!(result[0].cve, Some("CVE-2023-99999".to_string()));
        assert_eq!(result[0].fixed_in, Some("1.5.0".to_string()));
        assert_eq!(result[0].affected_versions, ">=1.0.0, <1.5.0");
        assert_eq!(result[0].references.len(), 2);
    }

    #[tokio::test]
    async fn test_query_package_multiple_affected_ranges() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": [{
                    "id": "CVE-2024-00001",
                    "summary": "Multi-range vuln",
                    "details": "Affects two ranges",
                    "affected": [{
                        "ranges": [{
                            "type": "SEMVER",
                            "events": [
                                {"introduced": "0"},
                                {"fixed": "2.0.0"},
                                {"introduced": "2.5.0"},
                                {"fixed": "3.0.0"}
                            ]
                        }]
                    }]
                }]
            })))
            .mount(&mock_server)
            .await;

        let mut api = OsvApi::new();
        api.base_url = mock_server.uri();

        let result = api.query_package("test", "1.0.0").await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].affected_versions,
            ">=0.0.0, <2.0.0 || >=2.5.0, <3.0.0"
        );
        assert_eq!(result[0].fixed_in, Some("2.0.0".to_string()));
    }

    #[tokio::test]
    async fn test_query_package_introduced_without_fixed() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "vulns": [{
                    "id": "CVE-2024-00002",
                    "summary": "Unfixed vuln",
                    "details": "No fix available",
                    "affected": [{
                        "ranges": [{
                            "type": "SEMVER",
                            "events": [
                                {"introduced": "1.0.0"}
                            ]
                        }]
                    }]
                }]
            })))
            .mount(&mock_server)
            .await;

        let mut api = OsvApi::new();
        api.base_url = mock_server.uri();

        let result = api.query_package("test", "2.0.0").await.unwrap();
        assert_eq!(result.len(), 1);
        // No fixed version, so all versions >= 1.0.0 are affected
        assert_eq!(result[0].affected_versions, ">=1.0.0");
        assert_eq!(result[0].fixed_in, None);
    }

    #[test]
    fn test_parse_osv_events_single_range() {
        let events = vec![
            OsvEvent {
                introduced: Some("0".to_string()),
                fixed: None,
            },
            OsvEvent {
                introduced: None,
                fixed: Some("2.0.0".to_string()),
            },
        ];
        let result = parse_osv_events(&events);
        assert!(result.is_some());
        let constraint = result.unwrap();
        assert!(matches!(constraint, VersionConstraint::Range { .. }));
    }

    #[test]
    fn test_parse_osv_events_empty() {
        let events: Vec<OsvEvent> = vec![];
        assert!(parse_osv_events(&events).is_none());
    }

    #[test]
    fn test_format_constraint_range() {
        let c = VersionConstraint::Range {
            lower: Version::new(1, 0, 0),
            upper: Version::new(2, 0, 0),
        };
        assert_eq!(format_constraint(&c), ">=1.0.0, <2.0.0");
    }

    #[test]
    fn test_format_constraint_any_of() {
        let c = VersionConstraint::AnyOf(vec![
            VersionConstraint::Range {
                lower: Version::new(0, 0, 0),
                upper: Version::new(2, 0, 0),
            },
            VersionConstraint::GreaterOrEqual(Version::new(3, 0, 0)),
        ]);
        assert_eq!(format_constraint(&c), ">=0.0.0, <2.0.0 || >=3.0.0");
    }
}
