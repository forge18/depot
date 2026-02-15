use crate::lua_analysis::compat_db::{format_version_hint, FeatureCategory};
use crate::lua_analysis::scanner::FileResult;
use crate::lua_analysis::LuaVersionSet;
use serde::Serialize;
use std::path::Path;

/// Aggregate compatibility report for an entire project.
#[derive(Debug)]
pub struct CompatibilityReport {
    pub files_scanned: usize,
    pub file_results: Vec<FileResult>,
    /// Overall project compatibility (intersection of all file results).
    pub project_compatible_versions: LuaVersionSet,
    /// The configured lua_version from package.yaml.
    pub configured_version: Option<String>,
    /// Whether the configured version is in the compatible set.
    pub config_compatible: Option<bool>,
}

/// Build a compatibility report from scan results.
pub fn build_report(
    file_results: Vec<FileResult>,
    configured_version: Option<&str>,
) -> CompatibilityReport {
    let files_scanned = file_results.len();

    let project_compatible_versions = if file_results.is_empty() {
        LuaVersionSet::all()
    } else {
        file_results.iter().fold(LuaVersionSet::all(), |acc, r| {
            acc.intersect(r.compatible_versions)
        })
    };

    let config_compatible = configured_version.map(|v| {
        // Extract major.minor from configured version (e.g., "5.4.8" -> "5.4")
        project_compatible_versions.contains_version_str(v)
    });

    CompatibilityReport {
        files_scanned,
        file_results,
        project_compatible_versions,
        configured_version: configured_version.map(|s| s.to_string()),
        config_compatible,
    }
}

/// Format the report as a human-readable string.
pub fn format_report(report: &CompatibilityReport, quiet: bool) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "\n  Compatible versions: {}\n",
        report.project_compatible_versions
    ));

    if let Some(ref configured) = report.configured_version {
        output.push_str(&format!("  Configured version:  {}\n", configured));
    }

    output.push_str(&format!("  Scanned: {} file(s)\n", report.files_scanned));

    // Collect all detected features across all files
    let mut all_features: Vec<(&Path, u32, &str, String, FeatureCategory)> = Vec::new();
    for result in &report.file_results {
        for feat in &result.detected_features {
            all_features.push((
                &result.path,
                feat.line,
                feat.info.name,
                format_version_hint(&feat.info),
                feat.info.category,
            ));
        }
    }

    if all_features.is_empty() {
        if !quiet {
            output.push_str(
                "\n  No version-specific features detected. Code is compatible with all Lua versions.\n",
            );
        }
    } else if !quiet || report.config_compatible == Some(false) {
        output.push_str("\n  Detected features:\n");

        for (path, line, name, hint, _category) in &all_features {
            let display_path = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Try to get a relative path for display
            let path_str = path.to_string_lossy();
            let display = if path_str.len() > 40 {
                display_path.to_string()
            } else {
                path_str.to_string()
            };

            output.push_str(&format!(
                "    {}:{:<8} {:<26} {}\n",
                display, line, name, hint
            ));
        }
    }

    // Compatibility verdict
    output.push('\n');
    match report.config_compatible {
        Some(true) => {
            if let Some(ref v) = report.configured_version {
                output.push_str(&format!(
                    "  Configured lua_version \"{}\" is compatible.\n",
                    v
                ));
            }
        }
        Some(false) => {
            if let Some(ref v) = report.configured_version {
                output.push_str(&format!(
                    "  Configured lua_version \"{}\" is INCOMPATIBLE with detected code.\n",
                    v
                ));

                if report.project_compatible_versions.is_empty() {
                    output.push_str(
                        "  Your code uses features from mutually exclusive Lua versions.\n",
                    );
                } else {
                    output.push_str(&format!(
                        "  Suggestion: Change lua_version to one of: {}\n",
                        report.project_compatible_versions
                    ));
                }
            }
        }
        None => {}
    }

    output
}

/// JSON-serializable report format.
#[derive(Debug, Serialize)]
pub struct JsonReport {
    pub files_scanned: usize,
    pub project_compatible_versions: Vec<&'static str>,
    pub configured_version: Option<String>,
    pub config_compatible: Option<bool>,
    pub features: Vec<JsonFeature>,
}

#[derive(Debug, Serialize)]
pub struct JsonFeature {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub name: String,
    pub category: String,
    pub available_in: Vec<&'static str>,
}

/// Convert a CompatibilityReport to a JSON-serializable format.
pub fn to_json_report(report: &CompatibilityReport) -> JsonReport {
    let mut features = Vec::new();
    for result in &report.file_results {
        for feat in &result.detected_features {
            features.push(JsonFeature {
                file: result.path.to_string_lossy().to_string(),
                line: feat.line,
                column: feat.column,
                name: feat.info.name.to_string(),
                category: match feat.info.category {
                    FeatureCategory::StdlibAdded => "stdlib_added".to_string(),
                    FeatureCategory::StdlibRemoved => "stdlib_removed".to_string(),
                    FeatureCategory::Syntax => "syntax".to_string(),
                    FeatureCategory::LuaJitExtension => "luajit_extension".to_string(),
                },
                available_in: feat.info.available_in.version_names(),
            });
        }
    }

    JsonReport {
        files_scanned: report.files_scanned,
        project_compatible_versions: report.project_compatible_versions.version_names(),
        configured_version: report.configured_version.clone(),
        config_compatible: report.config_compatible,
        features,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua_analysis::scanner::DetectedFeature;
    use std::path::PathBuf;

    fn make_file_result(features: Vec<(&'static str, LuaVersionSet)>) -> FileResult {
        let detected: Vec<DetectedFeature> = features
            .into_iter()
            .enumerate()
            .map(|(i, (name, versions))| DetectedFeature {
                info: crate::lua_analysis::compat_db::FeatureInfo {
                    name,
                    available_in: versions,
                    category: FeatureCategory::StdlibAdded,
                },
                line: (i + 1) as u32,
                column: 1,
            })
            .collect();

        let compat = detected.iter().fold(LuaVersionSet::all(), |acc, f| {
            acc.intersect(f.info.available_in)
        });

        FileResult {
            path: PathBuf::from("test.lua"),
            detected_features: detected,
            compatible_versions: compat,
        }
    }

    #[test]
    fn test_build_report_empty() {
        let report = build_report(vec![], Some("5.4"));
        assert_eq!(report.files_scanned, 0);
        assert_eq!(report.project_compatible_versions, LuaVersionSet::all());
        assert_eq!(report.config_compatible, Some(true));
    }

    #[test]
    fn test_build_report_compatible() {
        let v53_plus = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
        );
        let result = make_file_result(vec![("table.move", v53_plus)]);
        let report = build_report(vec![result], Some("5.4"));

        assert_eq!(report.config_compatible, Some(true));
        assert!(report
            .project_compatible_versions
            .contains(LuaVersionSet::LUA_5_4));
    }

    #[test]
    fn test_build_report_incompatible() {
        let v51_only = LuaVersionSet::from_bits(LuaVersionSet::LUA_5_1);
        let result = make_file_result(vec![("setfenv", v51_only)]);
        let report = build_report(vec![result], Some("5.4"));

        assert_eq!(report.config_compatible, Some(false));
    }

    #[test]
    fn test_format_report_compatible() {
        let v53_plus = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
        );
        let result = make_file_result(vec![("table.move", v53_plus)]);
        let report = build_report(vec![result], Some("5.4"));
        let output = format_report(&report, false);

        assert!(output.contains("Compatible versions: 5.3, 5.4, 5.5"));
        assert!(output.contains("table.move"));
        assert!(output.contains("is compatible"));
    }

    #[test]
    fn test_format_report_incompatible() {
        let v51_only = LuaVersionSet::from_bits(LuaVersionSet::LUA_5_1);
        let result = make_file_result(vec![("setfenv", v51_only)]);
        let report = build_report(vec![result], Some("5.4"));
        let output = format_report(&report, false);

        assert!(output.contains("INCOMPATIBLE"));
    }

    #[test]
    fn test_json_report() {
        let v53_plus = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
        );
        let result = make_file_result(vec![("table.move", v53_plus)]);
        let report = build_report(vec![result], Some("5.4"));
        let json = to_json_report(&report);

        assert_eq!(json.features.len(), 1);
        assert_eq!(json.features[0].name, "table.move");
        assert_eq!(json.config_compatible, Some(true));
    }
}
