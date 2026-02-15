# Development Setup

Complete guide to setting up a development environment for Depot.

## Prerequisites

### Required

- **Rust**: Latest stable version ([rustup.rs](https://rustup.rs/))
- **Lua**: 5.1, 5.3, or 5.4 installed
- **Git**: For version control

### Optional (for cross-compilation)

- **Zig**: For macOS/Linux cross-compilation (`brew install zig` on macOS)
- **cargo-zigbuild**: Installed automatically by build scripts
- **cargo-xwin**: For Windows cross-compilation (installed automatically)
- **clang**: Required for Windows builds (`brew install llvm` on macOS)

## Initial Setup

### 1. Clone the Repository

```bash
cd depot
```

### 2. Install Dependencies

Rust dependencies are managed by Cargo and will be installed automatically:

```bash
cargo build
```

### 3. Verify Installation

```bash
# Build
cargo build

# Run tests
cargo test

# Run Depot
cargo run -- --version
```

## Development Commands

### Build

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Run

```bash
# Run a command
cargo run -- install luasocket

# Run with debug output
RUST_LOG=debug cargo run -- install
```

### Test

```bash
# All tests
cargo test

# Specific test
cargo test test_name

# Integration tests
cargo test --test integration_tests

# Security tests
cargo test --test security

# With output
cargo test -- --nocapture
```

### Lint and Format

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy

# Clippy with all warnings
cargo clippy -- -W clippy::all
```

### Benchmarks

```bash
cargo bench
```

## IDE Setup

### VS Code

Recommended extensions:
- `rust-lang.rust-analyzer` - Rust language support
- `vadimcn.vscode-lldb` - Debugging

### IntelliJ / CLion

- Install Rust plugin
- Configure Rust toolchain

## Debugging

### Using `dbg!` macro

```rust
let value = some_function();
dbg!(&value);  // Prints to stderr
```

### Using a debugger

VS Code:
1. Set breakpoints
2. Press F5 to start debugging
3. Use debug console

Command line (gdb/lldb):
```bash
# Build with debug symbols
cargo build

# Run with debugger
lldb target/debug/depot
```

## Testing Locally

### Test Installation

```bash
# Build release
cargo build --release

# Install locally
cargo install --path .

# Test
depot --version
```

### Test Cross-Compilation

```bash
# Test macOS build
./scripts/build-installer.sh macos

# Test Linux build
./scripts/build-installer.sh linux

# Test Windows build
./scripts/build-installer.sh windows
```

## Common Issues

### "cannot find crate"

Run:
```bash
cargo clean
cargo build
```

### Tests fail

Check:
1. Lua is installed and in PATH
2. Test dependencies are available
3. No file permission issues

### Build fails on macOS

Ensure Xcode Command Line Tools are installed:
```bash
xcode-select --install
```

## Next Steps

- Read [Architecture](Architecture) to understand the codebase
- Check [Testing](Testing) for testing guidelines
- Review [Contributing](Contributing) for contribution guidelines

