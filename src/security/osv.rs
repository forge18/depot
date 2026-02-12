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
    vulns: Vec<OsvVulnerability>,
}

#[derive(Deserialize)]
struct OsvVulnerability {
    id: String,
    summary: String,
    details: String,
    #[serde(default)]
    severity: Vec<OsvSeverity>,
}

#[derive(Deserialize)]
struct OsvSeverity {
    #[serde(rename = "type")]
    severity_type: String,
    score: String,
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

    /// Query OSV API for vulnerabilities in a package version
    /// Returns empty vector if no vulnerabilities found or on API errors (non-fatal)
    pub async fn query_package(
        &self,
        name: &str,
        version: &str,
    ) -> DepotResult<Vec<Vulnerability>> {
        let query = OsvQuery {
            package: OsvPackage {
                // Check OSV docs for correct ecosystem name - try "LuaRocks" first
                ecosystem: "LuaRocks".to_string(),
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

        // Non-200 responses mean no vulnerabilities (or API issue - treat as none)
        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let osv_response: OsvResponse = response
            .json()
            .await
            .map_err(|e| DepotError::LuaRocks(format!("OSV parse error: {}", e)))?;

        Ok(osv_response
            .vulns
            .into_iter()
            .map(|v| {
                // Parse severity from OSV response (use highest if multiple)
                let severity = v
                    .severity
                    .iter()
                    .find(|s| s.severity_type == "CVSS_V3")
                    .or_else(|| v.severity.first())
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
                    .unwrap_or(Severity::Medium);

                Vulnerability {
                    package: name.to_string(),
                    affected_versions: version.to_string(),
                    severity,
                    title: v.summary,
                    description: v.details,
                    cve: Some(v.id),
                    fixed_in: None,
                    references: Vec::new(),
                }
            })
            .collect())
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
                ecosystem: "LuaRocks".to_string(),
                name: "test-package".to_string(),
            },
            version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("test-package"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("LuaRocks"));
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

        // Mock response with vulnerabilities
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
}
