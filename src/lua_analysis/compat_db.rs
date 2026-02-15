use super::LuaVersionSet;

/// A detected feature with its version compatibility information.
#[derive(Debug, Clone)]
pub struct FeatureInfo {
    /// Human-readable name (e.g., "table.move", "goto")
    pub name: &'static str,
    /// Set of Lua versions where this feature is available
    pub available_in: LuaVersionSet,
    /// Category for reporting
    pub category: FeatureCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureCategory {
    /// Standard library function/constant added in a specific version
    StdlibAdded,
    /// Standard library function removed in a later version
    StdlibRemoved,
    /// Syntax feature (operators, statements)
    Syntax,
    /// LuaJIT-specific extension
    LuaJitExtension,
}

// Version set helpers

fn v51_only() -> LuaVersionSet {
    LuaVersionSet::from_bits(LuaVersionSet::LUA_5_1)
}

fn v51_and_jit() -> LuaVersionSet {
    LuaVersionSet::from_bits(LuaVersionSet::LUA_5_1 | LuaVersionSet::LUAJIT)
}

fn v51_and_52() -> LuaVersionSet {
    LuaVersionSet::from_bits(LuaVersionSet::LUA_5_1 | LuaVersionSet::LUA_5_2)
}

fn v52_plus() -> LuaVersionSet {
    LuaVersionSet::from_bits(
        LuaVersionSet::LUA_5_2
            | LuaVersionSet::LUA_5_3
            | LuaVersionSet::LUA_5_4
            | LuaVersionSet::LUA_5_5,
    )
}

fn v52_plus_jit() -> LuaVersionSet {
    LuaVersionSet::from_bits(
        LuaVersionSet::LUA_5_2
            | LuaVersionSet::LUA_5_3
            | LuaVersionSet::LUA_5_4
            | LuaVersionSet::LUA_5_5
            | LuaVersionSet::LUAJIT,
    )
}

fn v53_plus() -> LuaVersionSet {
    LuaVersionSet::from_bits(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    )
}

fn v54_plus() -> LuaVersionSet {
    LuaVersionSet::from_bits(LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5)
}

fn v55_only() -> LuaVersionSet {
    LuaVersionSet::from_bits(LuaVersionSet::LUA_5_5)
}

fn jit_only() -> LuaVersionSet {
    LuaVersionSet::from_bits(LuaVersionSet::LUAJIT)
}

fn feature(
    name: &'static str,
    available_in: LuaVersionSet,
    category: FeatureCategory,
) -> FeatureInfo {
    FeatureInfo {
        name,
        available_in,
        category,
    }
}

/// Look up a dotted function/constant name like "table.move" or "math.maxinteger".
///
/// Returns `None` if the name is not a known version-specific feature
/// (i.e., it's available in all versions or is user-defined).
pub fn lookup_function(name: &str) -> Option<FeatureInfo> {
    use FeatureCategory::*;

    match name {
        // ===== Functions ADDED in 5.2 (not in 5.1) =====
        "table.pack" => Some(feature("table.pack", v52_plus(), StdlibAdded)),
        "table.unpack" => Some(feature("table.unpack", v52_plus(), StdlibAdded)),
        "rawlen" => Some(feature("rawlen", v52_plus(), StdlibAdded)),
        "package.searchpath" => Some(feature("package.searchpath", v52_plus(), StdlibAdded)),

        // ===== Functions ADDED in 5.3 (not in 5.2) =====
        "table.move" => Some(feature("table.move", v53_plus(), StdlibAdded)),
        "string.pack" => Some(feature("string.pack", v53_plus(), StdlibAdded)),
        "string.unpack" => Some(feature("string.unpack", v53_plus(), StdlibAdded)),
        "string.packsize" => Some(feature("string.packsize", v53_plus(), StdlibAdded)),
        "math.tointeger" => Some(feature("math.tointeger", v53_plus(), StdlibAdded)),
        "math.type" => Some(feature("math.type", v53_plus(), StdlibAdded)),
        "math.maxinteger" => Some(feature("math.maxinteger", v53_plus(), StdlibAdded)),
        "math.mininteger" => Some(feature("math.mininteger", v53_plus(), StdlibAdded)),
        "utf8.char" => Some(feature("utf8.char", v53_plus(), StdlibAdded)),
        "utf8.codepoint" => Some(feature("utf8.codepoint", v53_plus(), StdlibAdded)),
        "utf8.codes" => Some(feature("utf8.codes", v53_plus(), StdlibAdded)),
        "utf8.len" => Some(feature("utf8.len", v53_plus(), StdlibAdded)),
        "utf8.offset" => Some(feature("utf8.offset", v53_plus(), StdlibAdded)),
        "utf8.charpattern" => Some(feature("utf8.charpattern", v53_plus(), StdlibAdded)),
        "coroutine.isyieldable" => Some(feature("coroutine.isyieldable", v53_plus(), StdlibAdded)),

        // ===== Functions ADDED in 5.4 (not in 5.3) =====
        "warn" => Some(feature("warn", v54_plus(), StdlibAdded)),
        "coroutine.close" => Some(feature("coroutine.close", v54_plus(), StdlibAdded)),

        // ===== Functions ADDED in 5.5 (not in 5.4) =====
        "table.create" => Some(feature("table.create", v55_only(), StdlibAdded)),

        // ===== Functions REMOVED in 5.2 (only in 5.1) =====
        "setfenv" => Some(feature("setfenv", v51_only(), StdlibRemoved)),
        "getfenv" => Some(feature("getfenv", v51_only(), StdlibRemoved)),
        "math.log10" => Some(feature("math.log10", v51_only(), StdlibRemoved)),
        "table.maxn" => Some(feature("table.maxn", v51_only(), StdlibRemoved)),
        "module" => Some(feature("module", v51_only(), StdlibRemoved)),

        // ===== Functions REMOVED in 5.2 (available in 5.1 + LuaJIT) =====
        "unpack" => Some(feature("unpack", v51_and_jit(), StdlibRemoved)),
        "loadstring" => Some(feature("loadstring", v51_and_jit(), StdlibRemoved)),

        // ===== bit32 library (5.2 only, removed in 5.3) =====
        name if name.starts_with("bit32.") => Some(feature("bit32.*", v51_and_52(), StdlibRemoved)),

        // ===== LuaJIT-specific extensions =====
        name if name.starts_with("ffi.") => Some(feature("ffi.*", jit_only(), LuaJitExtension)),
        name if name.starts_with("bit.") => Some(feature("bit.*", jit_only(), LuaJitExtension)),
        name if name.starts_with("jit.") => Some(feature("jit.*", jit_only(), LuaJitExtension)),

        _ => None,
    }
}

/// Syntax feature: goto statement (Lua 5.2+, LuaJIT)
pub const SYNTAX_GOTO: FeatureInfo = FeatureInfo {
    name: "goto",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_2
            | LuaVersionSet::LUA_5_3
            | LuaVersionSet::LUA_5_4
            | LuaVersionSet::LUA_5_5
            | LuaVersionSet::LUAJIT,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: label statement (Lua 5.2+, LuaJIT)
pub const SYNTAX_LABEL: FeatureInfo = FeatureInfo {
    name: "::label::",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_2
            | LuaVersionSet::LUA_5_3
            | LuaVersionSet::LUA_5_4
            | LuaVersionSet::LUA_5_5
            | LuaVersionSet::LUAJIT,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: integer division // (Lua 5.3+)
pub const SYNTAX_INTEGER_DIVIDE: FeatureInfo = FeatureInfo {
    name: "// (integer division)",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: bitwise AND & (Lua 5.3+)
pub const SYNTAX_BITWISE_AND: FeatureInfo = FeatureInfo {
    name: "& (bitwise AND)",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: bitwise OR | (Lua 5.3+)
pub const SYNTAX_BITWISE_OR: FeatureInfo = FeatureInfo {
    name: "| (bitwise OR)",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: bitwise XOR ~ (Lua 5.3+)
pub const SYNTAX_BITWISE_XOR: FeatureInfo = FeatureInfo {
    name: "~ (bitwise XOR)",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: bitwise NOT ~ (Lua 5.3+)
pub const SYNTAX_BITWISE_NOT: FeatureInfo = FeatureInfo {
    name: "~ (bitwise NOT)",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: left shift << (Lua 5.3+)
pub const SYNTAX_SHIFT_LEFT: FeatureInfo = FeatureInfo {
    name: "<< (left shift)",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    ),
    category: FeatureCategory::Syntax,
};

/// Syntax feature: right shift >> (Lua 5.3+)
pub const SYNTAX_SHIFT_RIGHT: FeatureInfo = FeatureInfo {
    name: ">> (right shift)",
    available_in: LuaVersionSet(
        LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
    ),
    category: FeatureCategory::Syntax,
};

/// Look up a `require()` module name to detect version-specific dependencies.
pub fn lookup_require(module_name: &str) -> Option<FeatureInfo> {
    use FeatureCategory::*;

    match module_name {
        "ffi" => Some(feature("require(\"ffi\")", jit_only(), LuaJitExtension)),
        "bit" => Some(feature("require(\"bit\")", jit_only(), LuaJitExtension)),
        "jit" => Some(feature("require(\"jit\")", jit_only(), LuaJitExtension)),
        "bit32" => Some(feature("require(\"bit32\")", v51_and_52(), StdlibRemoved)),
        "utf8" => Some(feature("require(\"utf8\")", v53_plus(), StdlibAdded)),
        _ => None,
    }
}

/// Format a FeatureInfo's version availability as a compact description.
///
/// Examples: "(5.3+)", "(5.1 only)", "(LuaJIT only)", "(5.2+, LuaJIT)"
pub fn format_version_hint(info: &FeatureInfo) -> String {
    let versions = info.available_in;

    // Check for common patterns for compact display
    if versions == v51_only() {
        return "(5.1 only)".to_string();
    }
    if versions == v51_and_jit() {
        return "(5.1, LuaJIT)".to_string();
    }
    if versions == v51_and_52() {
        return "(5.1, 5.2 only)".to_string();
    }
    if versions == jit_only() {
        return "(LuaJIT only)".to_string();
    }
    if versions == v55_only() {
        return "(5.5 only)".to_string();
    }
    if versions == v52_plus() {
        return "(5.2+)".to_string();
    }
    if versions == v52_plus_jit() {
        return "(5.2+, LuaJIT)".to_string();
    }
    if versions == v53_plus() {
        return "(5.3+)".to_string();
    }
    if versions == v54_plus() {
        return "(5.4+)".to_string();
    }

    // Fallback: list all versions
    format!("({})", versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_added_functions() {
        let info = lookup_function("table.move").unwrap();
        assert_eq!(info.category, FeatureCategory::StdlibAdded);
        assert!(info.available_in.contains(LuaVersionSet::LUA_5_3));
        assert!(info.available_in.contains(LuaVersionSet::LUA_5_4));
        assert!(info.available_in.contains(LuaVersionSet::LUA_5_5));
        assert!(!info.available_in.contains(LuaVersionSet::LUA_5_1));
        assert!(!info.available_in.contains(LuaVersionSet::LUA_5_2));
    }

    #[test]
    fn test_lookup_removed_functions() {
        let info = lookup_function("setfenv").unwrap();
        assert_eq!(info.category, FeatureCategory::StdlibRemoved);
        assert!(info.available_in.contains(LuaVersionSet::LUA_5_1));
        assert!(!info.available_in.contains(LuaVersionSet::LUA_5_2));
    }

    #[test]
    fn test_lookup_luajit_extension() {
        let info = lookup_function("ffi.new").unwrap();
        assert_eq!(info.category, FeatureCategory::LuaJitExtension);
        assert!(info.available_in.contains(LuaVersionSet::LUAJIT));
        assert!(!info.available_in.contains(LuaVersionSet::LUA_5_4));
    }

    #[test]
    fn test_lookup_bit32() {
        let info = lookup_function("bit32.band").unwrap();
        assert_eq!(info.category, FeatureCategory::StdlibRemoved);
        assert!(info.available_in.contains(LuaVersionSet::LUA_5_1));
        assert!(info.available_in.contains(LuaVersionSet::LUA_5_2));
        assert!(!info.available_in.contains(LuaVersionSet::LUA_5_3));
    }

    #[test]
    fn test_lookup_unknown_returns_none() {
        assert!(lookup_function("print").is_none());
        assert!(lookup_function("my_custom_function").is_none());
        assert!(lookup_function("table.insert").is_none());
    }

    #[test]
    fn test_lookup_require() {
        let info = lookup_require("ffi").unwrap();
        assert_eq!(info.category, FeatureCategory::LuaJitExtension);
        assert!(info.available_in.contains(LuaVersionSet::LUAJIT));

        let info = lookup_require("utf8").unwrap();
        assert_eq!(info.category, FeatureCategory::StdlibAdded);
        assert!(info.available_in.contains(LuaVersionSet::LUA_5_3));

        assert!(lookup_require("socket").is_none());
    }

    #[test]
    fn test_format_version_hint() {
        let info = lookup_function("table.move").unwrap();
        assert_eq!(format_version_hint(&info), "(5.3+)");

        let info = lookup_function("setfenv").unwrap();
        assert_eq!(format_version_hint(&info), "(5.1 only)");

        let info = lookup_function("ffi.new").unwrap();
        assert_eq!(format_version_hint(&info), "(LuaJIT only)");

        assert_eq!(format_version_hint(&SYNTAX_GOTO), "(5.2+, LuaJIT)");
        assert_eq!(format_version_hint(&SYNTAX_INTEGER_DIVIDE), "(5.3+)");
    }

    #[test]
    fn test_54_functions() {
        let info = lookup_function("warn").unwrap();
        assert_eq!(format_version_hint(&info), "(5.4+)");

        let info = lookup_function("coroutine.close").unwrap();
        assert_eq!(format_version_hint(&info), "(5.4+)");
    }

    #[test]
    fn test_55_functions() {
        let info = lookup_function("table.create").unwrap();
        assert_eq!(format_version_hint(&info), "(5.5 only)");
    }
}
