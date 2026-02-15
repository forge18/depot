pub mod compat_db;
pub mod report;
pub mod scanner;

use std::fmt;

/// Bitflags representing a set of Lua versions.
///
/// Each bit corresponds to a specific Lua version. Intersecting two sets
/// gives the versions compatible with both requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuaVersionSet(u8);

impl LuaVersionSet {
    pub const LUA_5_1: u8 = 0b0000_0001;
    pub const LUA_5_2: u8 = 0b0000_0010;
    pub const LUA_5_3: u8 = 0b0000_0100;
    pub const LUA_5_4: u8 = 0b0000_1000;
    pub const LUA_5_5: u8 = 0b0001_0000;
    pub const LUAJIT: u8 = 0b0010_0000;

    pub const ALL_STANDARD: u8 =
        Self::LUA_5_1 | Self::LUA_5_2 | Self::LUA_5_3 | Self::LUA_5_4 | Self::LUA_5_5;
    pub const ALL: u8 = Self::ALL_STANDARD | Self::LUAJIT;

    pub fn all() -> Self {
        Self(Self::ALL)
    }

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn contains(self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns human-readable version names for all versions in this set.
    pub fn version_names(self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.contains(Self::LUA_5_1) {
            names.push("5.1");
        }
        if self.contains(Self::LUA_5_2) {
            names.push("5.2");
        }
        if self.contains(Self::LUA_5_3) {
            names.push("5.3");
        }
        if self.contains(Self::LUA_5_4) {
            names.push("5.4");
        }
        if self.contains(Self::LUA_5_5) {
            names.push("5.5");
        }
        if self.contains(Self::LUAJIT) {
            names.push("LuaJIT");
        }
        names
    }

    /// Checks if a specific Lua version string (e.g., "5.4") is in this set.
    pub fn contains_version_str(&self, version: &str) -> bool {
        match version {
            "5.1" => self.contains(Self::LUA_5_1),
            "5.2" => self.contains(Self::LUA_5_2),
            "5.3" => self.contains(Self::LUA_5_3),
            "5.4" => self.contains(Self::LUA_5_4),
            "5.5" => self.contains(Self::LUA_5_5),
            v if v.starts_with("5.1.") => self.contains(Self::LUA_5_1),
            v if v.starts_with("5.2.") => self.contains(Self::LUA_5_2),
            v if v.starts_with("5.3.") => self.contains(Self::LUA_5_3),
            v if v.starts_with("5.4.") => self.contains(Self::LUA_5_4),
            v if v.starts_with("5.5.") => self.contains(Self::LUA_5_5),
            _ => false,
        }
    }
}

impl fmt::Display for LuaVersionSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self.version_names();
        if names.is_empty() {
            write!(f, "(none)")
        } else {
            write!(f, "{}", names.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_contains_every_version() {
        let all = LuaVersionSet::all();
        assert!(all.contains(LuaVersionSet::LUA_5_1));
        assert!(all.contains(LuaVersionSet::LUA_5_2));
        assert!(all.contains(LuaVersionSet::LUA_5_3));
        assert!(all.contains(LuaVersionSet::LUA_5_4));
        assert!(all.contains(LuaVersionSet::LUA_5_5));
        assert!(all.contains(LuaVersionSet::LUAJIT));
    }

    #[test]
    fn test_empty() {
        let empty = LuaVersionSet::empty();
        assert!(empty.is_empty());
        assert!(!empty.contains(LuaVersionSet::LUA_5_1));
        assert_eq!(empty.version_names(), Vec::<&str>::new());
    }

    #[test]
    fn test_intersect() {
        let a = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
        );
        let b = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_2 | LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4,
        );
        let result = a.intersect(b);
        assert_eq!(
            result,
            LuaVersionSet::from_bits(LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4)
        );
    }

    #[test]
    fn test_union() {
        let a = LuaVersionSet::from_bits(LuaVersionSet::LUA_5_1);
        let b = LuaVersionSet::from_bits(LuaVersionSet::LUAJIT);
        let result = a.union(b);
        assert!(result.contains(LuaVersionSet::LUA_5_1));
        assert!(result.contains(LuaVersionSet::LUAJIT));
        assert!(!result.contains(LuaVersionSet::LUA_5_3));
    }

    #[test]
    fn test_version_names() {
        let set = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_1 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUAJIT,
        );
        assert_eq!(set.version_names(), vec!["5.1", "5.4", "LuaJIT"]);
    }

    #[test]
    fn test_display() {
        let set = LuaVersionSet::from_bits(LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4);
        assert_eq!(format!("{}", set), "5.3, 5.4");

        let empty = LuaVersionSet::empty();
        assert_eq!(format!("{}", empty), "(none)");
    }

    #[test]
    fn test_contains_version_str() {
        let set = LuaVersionSet::from_bits(LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5);
        assert!(set.contains_version_str("5.4"));
        assert!(set.contains_version_str("5.4.8"));
        assert!(set.contains_version_str("5.5"));
        assert!(!set.contains_version_str("5.3"));
        assert!(!set.contains_version_str("5.1"));
    }

    #[test]
    fn test_intersect_narrows_versions() {
        // Simulating: code uses table.move (5.3+) AND goto (5.2+)
        let table_move = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
        );
        let goto = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_2
                | LuaVersionSet::LUA_5_3
                | LuaVersionSet::LUA_5_4
                | LuaVersionSet::LUA_5_5
                | LuaVersionSet::LUAJIT,
        );

        let mut compat = LuaVersionSet::all();
        compat = compat.intersect(table_move);
        compat = compat.intersect(goto);

        assert_eq!(compat.version_names(), vec!["5.3", "5.4", "5.5"]);
    }

    #[test]
    fn test_contradictory_features_yield_empty() {
        // setfenv (5.1 only) + table.move (5.3+) = impossible
        let setfenv = LuaVersionSet::from_bits(LuaVersionSet::LUA_5_1);
        let table_move = LuaVersionSet::from_bits(
            LuaVersionSet::LUA_5_3 | LuaVersionSet::LUA_5_4 | LuaVersionSet::LUA_5_5,
        );

        let result = setfenv.intersect(table_move);
        assert!(result.is_empty());
    }
}
