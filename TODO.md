# TODO List for Depot

## Unimplemented Features

### ~~1. Security Advisory Database Integration~~ (DONE)

- **Status**: Completed. OSV integration fully implemented with proper affected version range parsing, CVSS severity scoring, CVE extraction from aliases, and consolidated code path through `SecurityAuditor`.


### ~~3. Remove depot-bundle Plugin~~ (DONE)

- **Status**: Completed. Experimental depot-bundle plugin has been removed from the codebase.
- **What was done**:
  - Removed `crates/depot-bundle/` directory
  - Removed from workspace `Cargo.toml`
  - Removed documentation in `docs/user/Plugins.md`
  - Removed from README.md plugins section
  - Removed references from all documentation files
  - Removed from GitHub Actions workflow
  - Removed from CHANGELOG.md

---

## Testing & Quality

### 8. Error Path Coverage

**Location**: Across test suite

- **Status**: Insufficient testing of failure scenarios
- **Description**: Limited tests for error conditions and edge cases
- **Impact**: Unknown behavior when errors occur in production

### 9. Test Coverage

**Location**: Across codebase

- **Status**: Currently 60% (measured by cargo-tarpaulin)
- **Description**: Coverage below industry standard of 70-80%
- **Impact**: Higher risk of undetected bugs in production

---

## Security Hardening

### 10. Release Checksums

**Location**: Release workflow

- **Status**: Not published
- **Description**: No SHA256 checksums published with releases
- **Impact**: Cannot verify download integrity

---

## Code Quality

### 11. Direct Process Exits

**Location**: Multiple CLI modules

- **Status**: Some places use `std::process::exit(1)`
- **Description**: Direct process exits instead of returning errors
- **Impact**: Difficult to test and handle errors gracefully
- **Example**: `src/cli/audit.rs:47`

### 12. Error Message Quality

**Location**: Across codebase

- **Status**: Many `.expect()` calls lack context messages
- **Description**: Error messages don't provide enough debugging context
- **Impact**: Difficult to diagnose issues when errors occur

---

## Production Readiness Checklist

### Must Have (Blocking v1.0)

- [ ] Remove "alpha" status from README
- [ ] Bump version to v1.0.0
- [x] Complete security advisory database (#1)
- [x] Remove depot-bundle plugin (#3)
- [ ] Increase test coverage to 70%+ (#9)

### Should Have (High Priority)

- [ ] Reduce panic/unwrap usage to <500 (#7)
- [ ] Add release checksums (#10)

### Nice to Have (Can Defer)

- [ ] Improve error message quality (#12)
- [ ] Replace direct process exits (#11)
