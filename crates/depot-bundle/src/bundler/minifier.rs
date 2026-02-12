use depot_core::DepotResult;
use full_moon::ast::Ast;
use full_moon::parse;

/// Lua minifier using parser for better results
pub struct Minifier;

impl Minifier {
    pub fn new() -> Self {
        Self
    }

    /// Minify Lua code using parser for better results
    ///
    /// Uses full_moon parser when possible, falls back to basic minification
    pub fn minify(&self, content: &str) -> DepotResult<String> {
        // Try to use parser-based minification
        if let Ok(ast) = parse(content) {
            return self.minify_with_ast(&ast, content);
        }

        // Fall back to basic minification
        self.minify_basic(content)
    }

    /// Minify using AST for better results.
    ///
    /// Currently falls back to basic minification as AST-based minification
    /// is not yet implemented. When implemented, this would provide better
    /// whitespace removal and code optimization.
    fn minify_with_ast(&self, _ast: &Ast, content: &str) -> DepotResult<String> {
        // AST-based minification not yet implemented - using basic minification
        self.minify_basic(content)
    }

    /// Strip comments only (preserve code structure)
    #[allow(dead_code)]
    pub fn strip_comments(&self, content: &str) -> DepotResult<String> {
        // Basic comment stripping
        let mut result = String::new();
        let mut in_multiline_comment = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle multiline comments
            if trimmed.contains("--[[") {
                in_multiline_comment = true;
            }
            if trimmed.contains("]]") {
                in_multiline_comment = false;
                continue;
            }
            if in_multiline_comment {
                continue;
            }

            // Skip single-line comments
            if trimmed.starts_with("--") && !trimmed.starts_with("--[[") {
                // Check if it's actually a comment or part of a string
                let mut chars = trimmed.chars();
                let mut in_string = false;
                let mut string_char = None;
                let mut is_comment = true;

                while let Some(ch) = chars.next() {
                    if !in_string && (ch == '"' || ch == '\'') {
                        in_string = true;
                        string_char = Some(ch);
                        is_comment = false;
                    } else if in_string && Some(ch) == string_char {
                        in_string = false;
                        string_char = None;
                    }

                    if !in_string && ch == '-' && chars.clone().next() == Some('-') {
                        break;
                    }
                }

                if is_comment {
                    continue;
                }
            }

            result.push_str(line);
            result.push('\n');
        }

        Ok(result)
    }

    /// Basic minification (fallback)
    fn minify_basic(&self, content: &str) -> DepotResult<String> {
        let mut result = String::new();
        let mut in_multiline_comment = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Handle multiline comments
            if trimmed.contains("--[[") {
                in_multiline_comment = true;
            }
            if trimmed.contains("]]") {
                in_multiline_comment = false;
                continue;
            }
            if in_multiline_comment {
                continue;
            }

            // Skip single-line comments (but preserve if in string)
            if trimmed.starts_with("--") && !trimmed.starts_with("--[[") {
                // Check if it's actually a comment or part of a string
                let mut chars = trimmed.chars();
                let mut is_comment = true;
                while let Some(ch) = chars.next() {
                    if ch == '"' || ch == '\'' {
                        is_comment = false;
                        break;
                    }
                    if ch == '-' && chars.next() == Some('-') {
                        break;
                    }
                }
                if is_comment {
                    continue;
                }
            }

            // Add line with minimal whitespace (preserve structure)
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(trimmed);
        }

        Ok(result)
    }
}
