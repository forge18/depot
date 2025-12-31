# Security

LPM is designed with security as a priority. This guide covers security features and best practices.

## Security Features

### Checksums

All packages are verified using BLAKE3 checksums stored in `lpm.lock`:

```yaml
packages:
  luasocket:
    version: "3.0.0"
    checksum: "blake3:abc123..."
```

BLAKE3 provides faster hashing with cryptographic security guarantees, making package verification both secure and performant.

Verify checksums:

```bash
lpm verify
```

### No Postinstall Scripts

LPM does not execute arbitrary code during installation. Packages are installed as-is, with no script execution.

### Sandboxed Builds

Rust extensions are built in sandboxed environments with restricted access to:
- File system
- Network
- System resources

### Secure Credential Storage

LPM stores LuaRocks credentials using OS keychains:
- **macOS**: Keychain
- **Windows**: Credential Manager
- **Linux**: Secret Service (libsecret)

```bash
lpm login  # Credentials stored securely
```

## Security Audit

Run security audits on your dependencies:

```bash
lpm audit
```

Checks for known vulnerabilities using:
- **OSV** (Open Source Vulnerabilities) - Primary source
- **GitHub Security Advisories** - Secondary source

### Audit Output

```
✓ No vulnerabilities found

or

⚠ Found 2 vulnerabilities:

1. luasocket@3.0.0
   Severity: HIGH
   CVE-2024-XXXXX: Buffer overflow in socket.connect
   Fixed in: 3.0.1
   Update: lpm update luasocket

2. penlight@1.12.0
   Severity: MEDIUM
   GHSA-XXXX: Path traversal vulnerability
   Fixed in: 1.13.0
   Update: lpm update penlight
```

## Best Practices

### 1. Use Lockfiles

Always commit `lpm.lock` to version control:

```bash
git add lpm.lock
git commit -m "Add lockfile"
```

This ensures:
- Reproducible builds
- Checksum verification
- Exact version pinning

### 2. Regular Updates

Keep dependencies updated:

```bash
# Check for updates
lpm outdated

# Update all dependencies
lpm update

# Run audit after updates
lpm audit
```

### 3. Version Constraints

Use specific version constraints:

```yaml
# Good: Specific version
dependencies:
  luasocket: "3.0.0"

# Better: Compatible version with upper bound
dependencies:
  luasocket: "^3.0.0"  # >=3.0.0 <4.0.0

# Avoid: Wildcard
dependencies:
  luasocket: "*"  # Too permissive
```

### 4. Verify Before Deploy

Always verify packages before deployment:

```bash
lpm verify
lpm audit
```

### 5. Review Dependencies

Regularly review your dependencies:

```bash
lpm list --tree
```

Remove unused dependencies:

```bash
lpm remove unused-package
```

### 6. Use Dev Dependencies

Separate development tools from production dependencies:

```yaml
dev_dependencies:
  busted: "^2.0.0"      # Test framework
  luacheck: "^1.0.0"     # Linter
```

Install production dependencies only:

```bash
lpm install --no-dev
```

## Supply Chain Security Checklist

- [ ] `lpm.lock` is committed to version control
- [ ] Regular security audits (`lpm audit`)
- [ ] Dependencies are kept up to date
- [ ] Version constraints are specific (not wildcards)
- [ ] Dev dependencies are separated
- [ ] Checksums are verified (`lpm verify`)
- [ ] Unused dependencies are removed
- [ ] Pre-built binaries are verified (if used)

## Reporting Vulnerabilities

If you discover a vulnerability in LPM:

1. **Do not** open a public issue
2. Email security@yourusername.github.io (or your security contact)
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

## Additional Resources

- [OSV Database](https://osv.dev/)
- [GitHub Security Advisories](https://github.com/advisories)
- [SemVer Specification](https://semver.org/)

