# TODO List for Depot

## Unimplemented Features

### 1. Security Advisory Database Integration
**File**: `src/security/advisory.rs:49`
- **Status**: Framework exists, external data loading not implemented
- **Description**: The advisory database exists but doesn't load vulnerability data from external sources like OSV (Open Source Vulnerabilities) or LuaRocks-specific databases
- **Impact**: Security auditing has limited real-world vulnerability data

### 2. AST-Based Lua Minification
**File**: `crates/depot-bundle/src/bundler/minifier.rs:31-34`
- **Status**: Falls back to basic minification
- **Description**: Parser-based minification using full_moon AST is stubbed out; currently uses regex-based basic minification only
- **Impact**: Bundle output is larger than it could be with proper AST optimization

### 3. depot-bundle Plugin
**File**: `crates/depot-bundle/` (entire crate)
- **Status**: Marked as "experimental"
- **Description**: Advanced bundling features like tree-shaking and AST-based minification are incomplete
- **Impact**: Bundle functionality works but is not production-ready

## Dead Code

### 1. Comment Stripping Function
**File**: `crates/depot-bundle/src/bundler/minifier.rs:37`
- Function `strip_comments()` marked with `#[allow(dead_code)]`
- Not currently used in the minification pipeline