use depot_core::{DepotError, DepotResult};
use full_moon::ast::Ast;
use full_moon::parse;

/// Lua parser for static analysis
pub struct LuaParser;

impl LuaParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse Lua code and extract require() calls
    ///
    /// Uses full_moon parser when possible, falls back to regex for compatibility
    pub fn extract_requires(&self, content: &str) -> DepotResult<Vec<String>> {
        // Try to parse with full_moon first
        if let Ok(ast) = parse(content) {
            return self.extract_requires_from_ast(&ast);
        }

        // Fall back to regex-based extraction
        self.extract_requires_regex(content)
    }

    /// Extract require() calls from parsed AST.
    ///
    /// Currently returns empty vector, causing fallback to regex-based extraction.
    /// AST-based extraction would provide more accurate results but is not yet implemented.
    fn extract_requires_from_ast(&self, _ast: &Ast) -> DepotResult<Vec<String>> {
        // AST-based extraction not yet implemented - falls back to regex
        Ok(Vec::new())
    }

    /// Fallback regex-based extraction (for when parsing fails)
    fn extract_requires_regex(&self, content: &str) -> DepotResult<Vec<String>> {
        use regex::Regex;
        let mut requires = Vec::new();

        // Match require("module") and require 'module'
        let re1 = Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#)
            .map_err(|e| DepotError::Package(format!("Invalid regex: {}", e)))?;
        for cap in re1.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                requires.push(m.as_str().to_string());
            }
        }

        // Match require[[module]] (long strings)
        let re2 = Regex::new(r#"require\s*\(\s*\[\[([^\]]+)\]\]\s*\)"#)
            .map_err(|e| DepotError::Package(format!("Invalid regex: {}", e)))?;
        for cap in re2.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                requires.push(m.as_str().to_string());
            }
        }

        // Match require 'module' (without parens)
        let re3 = Regex::new(r#"\brequire\s+['"]([^'"]+)['"]"#)
            .map_err(|e| DepotError::Package(format!("Invalid regex: {}", e)))?;
        for cap in re3.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                requires.push(m.as_str().to_string());
            }
        }

        Ok(requires)
    }
}
