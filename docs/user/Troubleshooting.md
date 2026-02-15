# Troubleshooting

Common issues and solutions when using Depot.

## Installation Issues

### "command not found: depot"

**Problem**: Depot is not in your PATH.

**Solution**:
```bash
# Run setup command
depot setup

# Or manually add to PATH
export PATH="$HOME/.cargo/bin:$PATH"
```

Then restart your terminal or reload your shell profile:
```bash
source ~/.bashrc  # or ~/.zshrc
```

### "Failed to find macOS SDK"

**Problem**: Xcode Command Line Tools not installed.

**Solution**:
```bash
xcode-select --install
```

### Windows: "clang not found"

**Problem**: `clang` is required for Windows cross-compilation.

**Solution**:
```bash
# On macOS
brew install llvm

# Verify installation
clang --version
```

## Dependency Issues

### "Package not found"

**Problem**: Package doesn't exist in LuaRocks or name is misspelled.

**Solution**:
1. Check package name on [LuaRocks](https://luarocks.org/)
2. Verify spelling: `depot install luasocket` (not `luasoket`)
3. Try searching: Check LuaRocks for exact package name

### "Version conflict"

**Problem**: Dependencies require incompatible versions.

**Solution**:
```bash
# See the conflict details
depot install

# Update to compatible versions
depot update <package>

# Or manually edit package.yaml to resolve
```

### "Checksum verification failed"

**Problem**: Package file was corrupted or tampered with.

**Solution**:
```bash
# Clean and reinstall
depot clean
depot install

# If problem persists, check network connection
# or try updating the package
depot update <package>
```

## Build Issues

### Rust Extension: "unable to find framework"

**Problem**: macOS frameworks not found during cross-compilation.

**Solution**: This is handled automatically by Depot. If it persists:
```bash
# Ensure SDK is available
xcrun --show-sdk-path

# Rebuild
depot build
```

### Rust Extension: "OpenSSL not found" (Linux)

**Problem**: OpenSSL required for native-tls.

**Solution**: Depot uses `rustls-tls` by default (no OpenSSL needed). If you see this error:
1. Check your `Cargo.toml` - ensure `reqwest` uses `rustls-tls`
2. Rebuild: `depot build`

### "Target not supported"

**Problem**: Trying to build for unsupported target.

**Solution**: Check supported targets:
```bash
depot build --help
```

Common targets:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## Runtime Issues

### "module 'X' not found"

**Problem**: Module not in `package.path`.

**Solution**:
1. Ensure you're using Depot's loader:
   ```lua
   require("depot.loader")  -- Sets up package.path
   local mymodule = require("mymodule")
   ```

2. Or use `depot exec`:
   ```bash
   depot exec lua main.lua
   ```

3. Verify package is installed:
   ```bash
   depot list
   ```

### "attempt to call a nil value"

**Problem**: Native module not found or wrong architecture.

**Solution**:
1. Check `package.cpath` includes `lua_modules/`
2. Verify native module exists:
   ```bash
   ls lua_modules/.depot/native/
   ```
3. Rebuild if needed:
   ```bash
   depot build
   ```

## Performance Issues

### Slow installs

**Problem**: Network or cache issues.

**Solution**:
```bash
# Clear cache
rm -rf ~/.cache/depot

# Try again
depot install
```

### Large lockfile

**Problem**: `depot.lock` is very large.

**Solution**: This is normal for projects with many dependencies. The lockfile ensures reproducibility.

## Getting Help

### Enable Debug Output

```bash
RUST_LOG=debug depot install
```

### Check Version

```bash
depot --version
```

### Verify Installation

```bash
# Check if Depot can find itself
which depot

# Check PATH
echo $PATH
```

## Common Error Messages

### "Already in an Depot project"

You're trying to run `depot init` in a directory that already has `package.yaml`.

**Solution**: Either remove `package.yaml` or work in a different directory.

### "Failed to resolve dependencies"

Dependency resolution failed due to conflicts.

**Solution**: 
```bash
# See detailed error
depot install

# Try updating
depot update

# Or manually resolve in package.yaml
```

### "Permission denied"

Depot doesn't have permission to write to `lua_modules/`.

**Solution**:
```bash
# Check permissions
ls -la

# Fix if needed
chmod -R u+w lua_modules/
```

## Still Having Issues?

1. Review the [CLI Commands](CLI-Commands) documentation
2. Check [Security](Security) for security-related issues
3. Open a new issue with:
   - Depot version (`depot --version`)
   - Error message
   - Steps to reproduce
   - Your `package.yaml` (if relevant)

