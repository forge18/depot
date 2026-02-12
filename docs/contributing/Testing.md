# Testing

Testing guidelines and practices for Depot.

## Test Organization

### Unit Tests

Unit tests are co-located with the code they test:

```rust
// src/package/manifest.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        // Test code
    }
}
```

Run unit tests:
```bash
cargo test
```

### Integration Tests

Integration tests are in `tests/`:

- **`integration_tests.rs`**: CLI command tests
- **`security.rs`**: Security-focused tests

Run integration tests:
```bash
cargo test --test integration_tests
cargo test --test security
```

### Benchmarks

Performance benchmarks are in `benches/`:

- **`performance_benchmarks.rs`**: Critical path benchmarks

Run benchmarks:
```bash
cargo bench
```

## Writing Tests

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    #[should_panic(expected = "Invalid version")]
    fn test_invalid_version() {
        Version::parse("invalid").unwrap();
    }
}
```

### Integration Test Example

```rust
// tests/integration_tests.rs
#[test]
fn test_lpm_init() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = Command::new("cargo")
        .args(&["run", "--", "init"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    
    assert!(output.status.success());
    assert!(temp_dir.path().join("package.yaml").exists());
}
```

## Test Coverage

### What to Test

- **Happy paths**: Normal operation
- **Error cases**: Invalid input, missing files, network errors
- **Edge cases**: Empty inputs, boundary values
- **Security**: Checksum verification, sandboxing

### What Not to Test

- Third-party library functionality
- Standard library behavior
- Obvious getters/setters (unless complex)

## Running Tests

### All Tests

```bash
cargo test
```

### Specific Test

```bash
cargo test test_name
```

### With Output

```bash
cargo test -- --nocapture
```

### Filter Tests

```bash
cargo test install  # Run tests with "install" in name
```

### Skip Doc Tests

```bash
cargo test --lib  # Only library tests
```

## Test Data

### Temporary Files

Use `tempfile` crate for temporary test data:

```rust
use tempfile::TempDir;

let temp_dir = TempDir::new().unwrap();
let file_path = temp_dir.path().join("test.yaml");
```

### Mocking

For external dependencies (HTTP, file system), consider:
- Dependency injection
- Test doubles
- Feature flags for test mode

## CI Testing

Tests run automatically on:
- Every push
- Every pull request
- Multiple platforms (Linux, macOS, Windows)

### Local CI Simulation

```bash
# Format check
cargo fmt --check

# Clippy
cargo clippy -- -D warnings

# Tests
cargo test

# Build
cargo build --release
```

## Performance Testing

### Benchmarks

Add benchmarks for critical paths:

```rust
// benches/performance_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_version_parse(c: &mut Criterion) {
    c.bench_function("parse_version", |b| {
        b.iter(|| Version::parse(black_box("1.2.3")))
    });
}
```

Run:
```bash
cargo bench
```

## Test Best Practices

1. **Test names**: Clear, descriptive names
2. **One assertion**: One concept per test
3. **Arrange-Act-Assert**: Structure tests clearly
4. **Test data**: Use realistic test data
5. **Cleanup**: Clean up test artifacts
6. **Isolation**: Tests should not depend on each other
7. **Fast**: Keep tests fast (use mocks for slow operations)

## Debugging Tests

### Print Debugging

```rust
#[test]
fn test_something() {
    let value = compute();
    dbg!(&value);  // Prints to stderr
    assert_eq!(value, expected);
}
```

### Test with Debugger

1. Set breakpoint in test
2. Run test in debugger
3. Step through code

## Continuous Testing

Consider using `cargo watch` for continuous testing:

```bash
cargo install cargo-watch
cargo watch -x test
```

Runs tests automatically on file changes.

