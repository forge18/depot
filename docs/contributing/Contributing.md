# Contributing to Depot

Thank you for your interest in contributing to Depot! This guide will help you get started.

## Getting Started

### Prerequisites

- Rust (latest stable) - [rustup.rs](https://rustup.rs/)
- Lua 5.1, 5.3, or 5.4 installed
- Git

### Development Setup

1. **Fork and clone the repository:**
   ```bash
   git clone https://github.com/yourusername/lpm.git
   cd lpm
   ```

2. **Build the project:**
   ```bash
   cargo build
   ```

3. **Run tests:**
   ```bash
   cargo test
   ```

4. **Set up pre-commit hooks (automatic):**
   
   Hooks are automatically installed when you build the project! RustyHook is configured in `.rusty-hook.toml` and will run formatting and linting checks before each commit.
   
   Just run:
   ```bash
   cargo build
   ```
   
   The hooks are now active!

5. **Run Depot locally:**
   ```bash
   cargo run -- install
   ```

## Development Workflow

### Making Changes

1. Create a new branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes

3. Run tests:
   ```bash
   cargo test
   ```

4. Run linter:
   ```bash
   cargo clippy
   ```

5. Format code:
   ```bash
   cargo fmt
   ```

6. Commit your changes:
   ```bash
   git commit -m "Add feature: description"
   ```

7. Push and create a pull request

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Use meaningful variable and function names
- Add comments for complex logic
- Write tests for new features
- Update documentation as needed

## Project Structure

```
lpm/
├── crates/
│   ├── depot-core/      # Core library (shared with plugins)
│   │   ├── src/
│   │   │   ├── core/  # Core utilities (errors, paths, version)
│   │   │   ├── package/# Package manifest
│   │   │   └── path_setup/# Lua path setup and runner
│   │   └── Cargo.toml
│   └── depot-watch/     # Watch mode plugin
│       └── src/
├── src/
│   ├── cli/           # CLI command implementations
│   │   └── plugin.rs  # Plugin discovery and execution
│   ├── core/          # Core functionality (errors, paths, version)
│   ├── package/       # Package management
│   ├── luarocks/      # LuaRocks integration
│   ├── resolver/      # Dependency resolution
│   ├── build/         # Rust extension building
│   ├── security/      # Security auditing
│   └── ...
├── tests/             # Integration tests
├── benches/           # Performance benchmarks
└── docs/              # Documentation
```

### Plugin Development

Depot supports plugins as separate executables. See [Plugin Development Guide](Plugin-Development) for details.

## Testing

### Unit Tests

Unit tests are in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function() {
        // Test code
    }
}
```

Run unit tests:
```bash
cargo test
```

### Integration Tests

Integration tests are in `tests/integration/`:

```bash
cargo test --test integration_tests
```

### E2E Tests

End-to-end tests are in `tests/e2e/` and test the full CLI workflow:

```bash
# Run all E2E tests (fast tests only, network tests are ignored by default)
cargo test --test e2e_tests

# Run network tests (requires network access)
cargo test --test e2e_tests -- --ignored

# Run specific test module
cargo test --test e2e_tests e2e::init

# Run specific test
cargo test --test e2e_tests e2e::init::init_creates_package_yaml
```

**Test Categories:**

- **Fast tests**: Run by default, no network required
- **Network tests**: Marked with `#[ignore = "requires network access"]`, use `--ignored` flag
- **Interactive tests**: Marked with `#[ignore = "requires terminal interaction"]`, require TTY

**Using the Test Runner Script:**

```bash
# Run all fast tests
./scripts/test-all.sh

# Run with network tests
./scripts/test-all.sh --network

# Run with interactive tests
./scripts/test-all.sh --interactive

# Run everything
./scripts/test-all.sh --all
```

**Skipping Network/Interactive Tests:**

Tests that require network access or terminal interaction are automatically skipped unless:
- You use the `--ignored` flag: `cargo test --test e2e_tests -- --ignored`
- You set environment variables:
  - `SKIP_NETWORK_TESTS=1` - Skip network tests
  - Tests check for TTY using `TestContext::require_tty()` which automatically skips in CI

**Test Isolation Strategy:**

Each test gets an isolated environment via `TestContext`:
- Temporary directory for each test (automatically cleaned up)
- Isolated Depot home directory (config and cache)
- Platform-specific environment variable isolation
- No shared state between tests

This means:
- Tests can run in parallel safely
- No `#[serial]` attributes needed
- Each test is completely independent

Example:

```rust
use super::*;

#[test]
fn my_test() {
    let ctx = TestContext::new(); // Isolated environment
    
    ctx.lpm()
        .arg("init")
        .arg("--yes")
        .assert()
        .success();
    
    // Test continues with isolated temp directory
    // Automatically cleaned up when ctx is dropped
}
```

### Security Tests

```bash
cargo test --test security
```

### Benchmarks

```bash
cargo bench
```

## Documentation

- User documentation: `docs/user/` (synced to wiki)
- Contributor documentation: `docs/contributing/` (synced to wiki)
- Code documentation: Inline with `///` comments

When adding features, update relevant documentation.

## Pull Request Process

1. **Before submitting:**
   - [ ] Code compiles without warnings
   - [ ] All tests pass
   - [ ] Code is formatted (`cargo fmt`)
   - [ ] No clippy warnings (`cargo clippy`)
   - [ ] Documentation updated
   - [ ] Commit messages are clear

2. **Create PR:**
   - Clear title and description
   - Reference related issues
   - Describe changes and motivation

3. **Review process:**
   - Address feedback
   - Keep PR focused (one feature/fix per PR)
   - Update PR if requested

## Areas for Contribution

- Bug fixes
- New features (check issues for ideas)
- Documentation improvements
- Performance optimizations
- Test coverage
- Cross-platform compatibility

## Questions?

- Open an issue for discussion
- Check existing issues and PRs
- Review [Architecture](Architecture) documentation

Thank you for contributing!

