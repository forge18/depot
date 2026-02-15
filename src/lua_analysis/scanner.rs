use crate::core::{DepotError, DepotResult};
use crate::lua_analysis::compat_db::{self, FeatureCategory, FeatureInfo};
use crate::lua_analysis::LuaVersionSet;
use luanext_parser::prelude::*;
use luanext_parser::{
    Bump, CollectingDiagnosticHandler, DiContainer, DiagnosticHandler, Lexer, Parser,
    ServiceLifetime, StringInterner,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

/// Result of scanning a single file.
#[derive(Debug)]
pub struct FileResult {
    pub path: PathBuf,
    pub detected_features: Vec<DetectedFeature>,
    /// Intersection of all feature compatibility sets.
    pub compatible_versions: LuaVersionSet,
}

/// A single detected version-specific feature with its location.
#[derive(Debug)]
pub struct DetectedFeature {
    pub info: FeatureInfo,
    pub line: u32,
    pub column: u32,
}

/// Scan all `.lua` files under a project root.
///
/// Skips `lua_modules/`, hidden directories, and files that fail to parse.
pub fn scan_project(project_root: &Path) -> DepotResult<Vec<FileResult>> {
    let mut results = Vec::new();
    let mut warnings = Vec::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden directories, lua_modules, and common non-source dirs
            // But always allow the root entry (depth 0)
            if e.file_type().is_dir() && e.depth() > 0 {
                return !name.starts_with('.') && name != "lua_modules" && name != "node_modules";
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "lua") {
            match scan_file(path) {
                Ok(result) => results.push(result),
                Err(e) => {
                    warnings.push(format!(
                        "  Warning: Failed to parse {}: {}",
                        path.strip_prefix(project_root).unwrap_or(path).display(),
                        e
                    ));
                }
            }
        }
    }

    // Print warnings for files that couldn't be parsed
    for warning in &warnings {
        eprintln!("{}", warning);
    }

    Ok(results)
}

/// Scan a single `.lua` file for version-specific features.
pub fn scan_file(path: &Path) -> DepotResult<FileResult> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| DepotError::Path(format!("Failed to read {}: {}", path.display(), e)))?;

    let detected = scan_source(&source)?;

    let mut compatible_versions = LuaVersionSet::all();
    for feat in &detected {
        compatible_versions = compatible_versions.intersect(feat.info.available_in);
    }

    Ok(FileResult {
        path: path.to_path_buf(),
        detected_features: detected,
        compatible_versions,
    })
}

/// Scan a source string for version-specific features.
fn scan_source(source: &str) -> DepotResult<Vec<DetectedFeature>> {
    let arena = Bump::new();
    let mut container = DiContainer::new();
    container.register(
        |_| Arc::new(CollectingDiagnosticHandler::new()) as Arc<dyn DiagnosticHandler>,
        ServiceLifetime::Transient,
    );

    let handler = container
        .resolve::<Arc<dyn DiagnosticHandler>>()
        .ok_or_else(|| DepotError::Package("Failed to create diagnostic handler".to_string()))?;

    let (interner, common) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);

    let tokens = lexer
        .tokenize()
        .map_err(|e| DepotError::Package(format!("Lexer error: {:?}", e)))?;

    let mut parser = Parser::new(tokens, handler, &interner, &common, &arena);
    let program = parser
        .parse()
        .map_err(|e| DepotError::Package(format!("Parse error: {}", e)))?;

    let mut detected = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for stmt in program.statements {
        walk_statement(stmt, &interner, &mut detected, &mut seen);
    }

    Ok(detected)
}

/// Resolve a callee expression to a dotted name like "table.move" or "setfenv".
fn resolve_dotted_name(expr: &Expression<'_>, interner: &StringInterner) -> Option<String> {
    match &expr.kind {
        ExpressionKind::Identifier(id) => Some(interner.resolve(*id)),
        ExpressionKind::Member(obj, ident) => {
            let obj_name = resolve_dotted_name(obj, interner)?;
            let member_name = interner.resolve(ident.node);
            Some(format!("{}.{}", obj_name, member_name))
        }
        _ => None,
    }
}

/// Check if a call expression is `require("module_name")` and return the module name.
fn extract_require_arg<'a>(
    callee: &Expression<'a>,
    args: &[Argument<'a>],
    interner: &StringInterner,
) -> Option<String> {
    // Check callee is the identifier "require"
    if let ExpressionKind::Identifier(id) = &callee.kind {
        if interner.resolve(*id) == "require" {
            // Check there's exactly one string literal argument
            if args.len() == 1 {
                if let ExpressionKind::Literal(Literal::String(s)) = &args[0].value.kind {
                    return Some(s.clone());
                }
            }
        }
    }
    None
}

/// Record a detected feature, deduplicating by (name, line).
fn record_feature(
    info: FeatureInfo,
    span: &luanext_parser::Span,
    detected: &mut Vec<DetectedFeature>,
    seen: &mut std::collections::HashSet<(String, u32)>,
) {
    let key = (info.name.to_string(), span.line);
    if seen.insert(key) {
        detected.push(DetectedFeature {
            info,
            line: span.line,
            column: span.column,
        });
    }
}

// ===== AST Walking =====

fn walk_statement(
    stmt: &Statement<'_>,
    interner: &StringInterner,
    detected: &mut Vec<DetectedFeature>,
    seen: &mut std::collections::HashSet<(String, u32)>,
) {
    match stmt {
        Statement::Goto(goto) => {
            record_feature(compat_db::SYNTAX_GOTO, &goto.span, detected, seen);
        }
        Statement::Label(label) => {
            record_feature(compat_db::SYNTAX_LABEL, &label.span, detected, seen);
        }
        Statement::Variable(decl) => {
            walk_expression(&decl.initializer, interner, detected, seen);
        }
        Statement::Function(decl) => {
            walk_block(&decl.body, interner, detected, seen);
        }
        Statement::If(stmt) => {
            walk_expression(&stmt.condition, interner, detected, seen);
            walk_block(&stmt.then_block, interner, detected, seen);
            for else_if in stmt.else_ifs.iter() {
                walk_expression(&else_if.condition, interner, detected, seen);
                walk_block(&else_if.block, interner, detected, seen);
            }
            if let Some(else_block) = &stmt.else_block {
                walk_block(else_block, interner, detected, seen);
            }
        }
        Statement::While(stmt) => {
            walk_expression(&stmt.condition, interner, detected, seen);
            walk_block(&stmt.body, interner, detected, seen);
        }
        Statement::For(for_stmt) => match for_stmt {
            ForStatement::Numeric(num) => {
                walk_expression(&num.start, interner, detected, seen);
                walk_expression(&num.end, interner, detected, seen);
                if let Some(step) = &num.step {
                    walk_expression(step, interner, detected, seen);
                }
                walk_block(&num.body, interner, detected, seen);
            }
            ForStatement::Generic(gen) => {
                for iter_expr in gen.iterators.iter() {
                    walk_expression(iter_expr, interner, detected, seen);
                }
                walk_block(&gen.body, interner, detected, seen);
            }
        },
        Statement::Repeat(stmt) => {
            walk_block(&stmt.body, interner, detected, seen);
            walk_expression(&stmt.until, interner, detected, seen);
        }
        Statement::Return(stmt) => {
            for value in stmt.values.iter() {
                walk_expression(value, interner, detected, seen);
            }
        }
        Statement::Expression(expr) => {
            walk_expression(expr, interner, detected, seen);
        }
        Statement::Block(block) => {
            walk_block(block, interner, detected, seen);
        }
        Statement::Class(decl) => {
            for member in decl.members.iter() {
                walk_class_member(member, interner, detected, seen);
            }
        }
        Statement::Try(try_stmt) => {
            walk_block(&try_stmt.try_block, interner, detected, seen);
            for clause in try_stmt.catch_clauses.iter() {
                walk_block(&clause.body, interner, detected, seen);
            }
            if let Some(finally) = &try_stmt.finally_block {
                walk_block(finally, interner, detected, seen);
            }
        }
        Statement::Throw(throw) => {
            walk_expression(&throw.expression, interner, detected, seen);
        }
        Statement::Export(export) => {
            if let ExportKind::Declaration(stmt) = &export.kind {
                walk_statement(stmt, interner, detected, seen);
            } else if let ExportKind::Default(expr) = &export.kind {
                walk_expression(expr, interner, detected, seen);
            }
        }
        // Statements that don't contain expressions or sub-statements we need to check
        Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Rethrow(_)
        | Statement::Import(_)
        | Statement::Interface(_)
        | Statement::TypeAlias(_)
        | Statement::Enum(_)
        | Statement::Namespace(_)
        | Statement::DeclareFunction(_)
        | Statement::DeclareNamespace(_)
        | Statement::DeclareType(_)
        | Statement::DeclareInterface(_)
        | Statement::DeclareConst(_) => {}
    }
}

fn walk_block(
    block: &Block<'_>,
    interner: &StringInterner,
    detected: &mut Vec<DetectedFeature>,
    seen: &mut std::collections::HashSet<(String, u32)>,
) {
    for stmt in block.statements.iter() {
        walk_statement(stmt, interner, detected, seen);
    }
}

fn walk_class_member(
    member: &ClassMember<'_>,
    interner: &StringInterner,
    detected: &mut Vec<DetectedFeature>,
    seen: &mut std::collections::HashSet<(String, u32)>,
) {
    match member {
        ClassMember::Method(method) => {
            if let Some(body) = &method.body {
                walk_block(body, interner, detected, seen);
            }
        }
        ClassMember::Constructor(ctor) => {
            walk_block(&ctor.body, interner, detected, seen);
        }
        ClassMember::Property(prop) => {
            if let Some(init) = &prop.initializer {
                walk_expression(init, interner, detected, seen);
            }
        }
        ClassMember::Getter(getter) => {
            walk_block(&getter.body, interner, detected, seen);
        }
        ClassMember::Setter(setter) => {
            walk_block(&setter.body, interner, detected, seen);
        }
        ClassMember::Operator(op) => {
            walk_block(&op.body, interner, detected, seen);
        }
    }
}

fn walk_expression(
    expr: &Expression<'_>,
    interner: &StringInterner,
    detected: &mut Vec<DetectedFeature>,
    seen: &mut std::collections::HashSet<(String, u32)>,
) {
    match &expr.kind {
        // Function calls — the primary detection mechanism
        ExpressionKind::Call(callee, args, _) => {
            // Check for require("module") calls
            if let Some(module_name) = extract_require_arg(callee, args, interner) {
                if let Some(info) = compat_db::lookup_require(&module_name) {
                    record_feature(info, &expr.span, detected, seen);
                }
            }

            // Check callee for version-specific function
            if let Some(name) = resolve_dotted_name(callee, interner) {
                if let Some(info) = compat_db::lookup_function(&name) {
                    record_feature(info, &expr.span, detected, seen);
                }
            }

            // Walk callee and args recursively
            walk_expression(callee, interner, detected, seen);
            for arg in args.iter() {
                walk_expression(&arg.value, interner, detected, seen);
            }
        }

        // Method calls: obj:method(...)
        ExpressionKind::MethodCall(receiver, _method, args, _) => {
            walk_expression(receiver, interner, detected, seen);
            for arg in args.iter() {
                walk_expression(&arg.value, interner, detected, seen);
            }
        }

        // Member access: check for version-specific constants (e.g., math.maxinteger)
        ExpressionKind::Member(obj, _ident) => {
            if let Some(name) = resolve_dotted_name(expr, interner) {
                if let Some(info) = compat_db::lookup_function(&name) {
                    // Only record constants (not function calls — those are caught by Call)
                    if info.category == FeatureCategory::StdlibAdded
                        || info.category == FeatureCategory::LuaJitExtension
                    {
                        record_feature(info, &expr.span, detected, seen);
                    }
                }
            }
            walk_expression(obj, interner, detected, seen);
        }

        // Binary operators — detect version-specific operators
        ExpressionKind::Binary(op, left, right) => {
            let syntax_feature = match op {
                BinaryOp::IntegerDivide => Some(compat_db::SYNTAX_INTEGER_DIVIDE),
                BinaryOp::BitwiseAnd => Some(compat_db::SYNTAX_BITWISE_AND),
                BinaryOp::BitwiseOr => Some(compat_db::SYNTAX_BITWISE_OR),
                BinaryOp::BitwiseXor => Some(compat_db::SYNTAX_BITWISE_XOR),
                BinaryOp::ShiftLeft => Some(compat_db::SYNTAX_SHIFT_LEFT),
                BinaryOp::ShiftRight => Some(compat_db::SYNTAX_SHIFT_RIGHT),
                _ => None,
            };
            if let Some(info) = syntax_feature {
                record_feature(info, &expr.span, detected, seen);
            }
            walk_expression(left, interner, detected, seen);
            walk_expression(right, interner, detected, seen);
        }

        // Unary operators
        ExpressionKind::Unary(op, operand) => {
            if *op == UnaryOp::BitwiseNot {
                record_feature(compat_db::SYNTAX_BITWISE_NOT, &expr.span, detected, seen);
            }
            walk_expression(operand, interner, detected, seen);
        }

        // Assignment — walk both sides
        ExpressionKind::Assignment(target, _op, value) => {
            walk_expression(target, interner, detected, seen);
            walk_expression(value, interner, detected, seen);
        }

        // Index: table[key]
        ExpressionKind::Index(obj, key) => {
            walk_expression(obj, interner, detected, seen);
            walk_expression(key, interner, detected, seen);
        }

        // Array literal
        ExpressionKind::Array(elements) => {
            for elem in elements.iter() {
                match elem {
                    ArrayElement::Expression(e) | ArrayElement::Spread(e) => {
                        walk_expression(e, interner, detected, seen);
                    }
                }
            }
        }

        // Object/table literal
        ExpressionKind::Object(props) => {
            for prop in props.iter() {
                match prop {
                    ObjectProperty::Property { value, .. } => {
                        walk_expression(value, interner, detected, seen);
                    }
                    ObjectProperty::Computed { key, value, .. } => {
                        walk_expression(key, interner, detected, seen);
                        walk_expression(value, interner, detected, seen);
                    }
                    ObjectProperty::Spread { value, .. } => {
                        walk_expression(value, interner, detected, seen);
                    }
                }
            }
        }

        // Function expression
        ExpressionKind::Function(func) => {
            walk_block(&func.body, interner, detected, seen);
        }

        // Arrow function
        ExpressionKind::Arrow(arrow) => match &arrow.body {
            ArrowBody::Expression(e) => walk_expression(e, interner, detected, seen),
            ArrowBody::Block(block) => walk_block(block, interner, detected, seen),
        },

        // Conditional (ternary)
        ExpressionKind::Conditional(cond, then, else_) => {
            walk_expression(cond, interner, detected, seen);
            walk_expression(then, interner, detected, seen);
            walk_expression(else_, interner, detected, seen);
        }

        // Parenthesized
        ExpressionKind::Parenthesized(inner) => {
            walk_expression(inner, interner, detected, seen);
        }

        // Pipe operator
        ExpressionKind::Pipe(left, right) => {
            walk_expression(left, interner, detected, seen);
            walk_expression(right, interner, detected, seen);
        }

        // Match expression
        ExpressionKind::Match(match_expr) => {
            walk_expression(match_expr.value, interner, detected, seen);
            for arm in match_expr.arms.iter() {
                if let Some(guard) = &arm.guard {
                    walk_expression(guard, interner, detected, seen);
                }
                match &arm.body {
                    MatchArmBody::Expression(e) => walk_expression(e, interner, detected, seen),
                    MatchArmBody::Block(block) => walk_block(block, interner, detected, seen),
                }
            }
        }

        // Type assertion
        ExpressionKind::TypeAssertion(inner, _) => {
            walk_expression(inner, interner, detected, seen);
        }

        // New expression
        ExpressionKind::New(callee, args, _) => {
            walk_expression(callee, interner, detected, seen);
            for arg in args.iter() {
                walk_expression(&arg.value, interner, detected, seen);
            }
        }

        // Optional chaining variants
        ExpressionKind::OptionalMember(obj, _) | ExpressionKind::OptionalIndex(obj, _) => {
            walk_expression(obj, interner, detected, seen);
        }
        ExpressionKind::OptionalCall(callee, args, _)
        | ExpressionKind::OptionalMethodCall(callee, _, args, _) => {
            walk_expression(callee, interner, detected, seen);
            for arg in args.iter() {
                walk_expression(&arg.value, interner, detected, seen);
            }
        }

        // Try expression
        ExpressionKind::Try(try_expr) => {
            walk_expression(try_expr.expression, interner, detected, seen);
            walk_expression(try_expr.catch_expression, interner, detected, seen);
        }

        // Error chain
        ExpressionKind::ErrorChain(left, right) => {
            walk_expression(left, interner, detected, seen);
            walk_expression(right, interner, detected, seen);
        }

        // Template literal
        ExpressionKind::Template(template) => {
            for part in template.parts.iter() {
                if let luanext_parser::ast::expression::TemplatePart::Expression(e) = part {
                    walk_expression(e, interner, detected, seen);
                }
            }
        }

        // Leaf nodes — nothing to walk
        ExpressionKind::Identifier(_)
        | ExpressionKind::Literal(_)
        | ExpressionKind::SelfKeyword
        | ExpressionKind::SuperKeyword => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(source: &str) -> Vec<DetectedFeature> {
        scan_source(source).unwrap()
    }

    fn feature_names(source: &str) -> Vec<String> {
        scan(source)
            .into_iter()
            .map(|f| f.info.name.to_string())
            .collect()
    }

    #[test]
    fn test_detect_table_move() {
        let features = feature_names("table.move(t, 1, #t, 2)");
        assert!(features.contains(&"table.move".to_string()));
    }

    #[test]
    fn test_detect_setfenv() {
        let features = feature_names("setfenv(1, env)");
        assert!(features.contains(&"setfenv".to_string()));
    }

    #[test]
    fn test_detect_string_pack() {
        let features = feature_names("local packed = string.pack('i4', 42)");
        assert!(features.contains(&"string.pack".to_string()));
    }

    #[test]
    fn test_detect_warn() {
        let features = feature_names("warn('something happened')");
        assert!(features.contains(&"warn".to_string()));
    }

    #[test]
    fn test_detect_goto() {
        let features = feature_names("goto done\n::done::");
        assert!(features.contains(&"goto".to_string()));
        assert!(features.contains(&"::label::".to_string()));
    }

    #[test]
    fn test_detect_integer_division() {
        let features = feature_names("local x = 10 // 3");
        assert!(features.contains(&"// (integer division)".to_string()));
    }

    #[test]
    fn test_detect_bitwise_and() {
        let features = feature_names("local x = a & b");
        assert!(features.contains(&"& (bitwise AND)".to_string()));
    }

    #[test]
    fn test_detect_bitwise_not() {
        let features = feature_names("local x = ~a");
        assert!(features.contains(&"~ (bitwise NOT)".to_string()));
    }

    #[test]
    fn test_detect_require_ffi() {
        let features = feature_names("local ffi = require(\"ffi\")");
        assert!(features.contains(&"require(\"ffi\")".to_string()));
    }

    #[test]
    fn test_detect_require_utf8() {
        let features = feature_names("local utf8 = require(\"utf8\")");
        assert!(features.contains(&"require(\"utf8\")".to_string()));
    }

    #[test]
    fn test_no_detection_in_comments() {
        // The parser won't include comments in the AST
        let features = feature_names("-- setfenv(1, env)");
        assert!(!features.contains(&"setfenv".to_string()));
    }

    #[test]
    fn test_no_detection_in_strings() {
        let features = feature_names("local s = 'setfenv is old'");
        assert!(!features.contains(&"setfenv".to_string()));
    }

    #[test]
    fn test_no_detection_for_common_functions() {
        let features = feature_names("print('hello')\ntable.insert(t, 1)");
        assert!(features.is_empty());
    }

    #[test]
    fn test_multiple_features_narrow_versions() {
        let result = scan_source("table.move(t, 1, #t, 2)\ngoto done\n::done::").unwrap();

        let mut compat = LuaVersionSet::all();
        for feat in &result {
            compat = compat.intersect(feat.info.available_in);
        }

        // table.move is 5.3+ and goto is 5.2+, LuaJIT
        // Intersection: 5.3, 5.4, 5.5
        assert!(compat.contains(LuaVersionSet::LUA_5_3));
        assert!(compat.contains(LuaVersionSet::LUA_5_4));
        assert!(compat.contains(LuaVersionSet::LUA_5_5));
        assert!(!compat.contains(LuaVersionSet::LUA_5_1));
        assert!(!compat.contains(LuaVersionSet::LUA_5_2));
        assert!(!compat.contains(LuaVersionSet::LUAJIT));
    }

    #[test]
    fn test_nested_function_call() {
        let features = feature_names("print(string.pack('i4', math.tointeger(x)))");
        assert!(features.contains(&"string.pack".to_string()));
        assert!(features.contains(&"math.tointeger".to_string()));
    }

    #[test]
    fn test_member_access_constant() {
        let features = feature_names("local x = math.maxinteger");
        assert!(features.contains(&"math.maxinteger".to_string()));
    }

    #[test]
    fn test_coroutine_close() {
        let features = feature_names("coroutine.close(co)");
        assert!(features.contains(&"coroutine.close".to_string()));
    }

    #[test]
    fn test_table_create() {
        let features = feature_names("local t = table.create(100, 0)");
        assert!(features.contains(&"table.create".to_string()));
    }

    #[test]
    fn test_shift_operators() {
        let features = feature_names("local x = a << 2\nlocal y = b >> 1");
        assert!(features.contains(&"<< (left shift)".to_string()));
        assert!(features.contains(&">> (right shift)".to_string()));
    }

    #[test]
    fn test_function_body_scanning() {
        let features = feature_names(
            r#"
            function foo()
                local x = table.move(t, 1, #t, 2)
                return x
            end
            "#,
        );
        assert!(features.contains(&"table.move".to_string()));
    }

    #[test]
    fn test_empty_source() {
        let features = feature_names("");
        assert!(features.is_empty());
    }
}
