# Contributing to LPM

Thank you for your interest in contributing to LPM! This document provides a quick overview. For detailed information, see [docs/contributing/Contributing.md](docs/contributing/Contributing.md).

## Quick Start

1. **Fork and clone the repository**
2. **Set up development environment:**
   ```bash
   cargo build
   cargo test
   ```
3. **Make your changes**
4. **Run checks:**
   ```bash
   cargo fmt
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ```
5. **Commit and push** (pre-commit hooks will run automatically)
6. **Open a pull request**

## Development Setup

See [docs/contributing/Development-Setup.md](docs/contributing/Development-Setup.md) for complete setup instructions.

## Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Fix all clippy warnings (`cargo clippy`)
- Write tests for new features
- Update documentation

## Testing

- Unit tests: `cargo test`
- Integration tests: `cargo test --test integration_tests`
- Security tests: `cargo test --test security`

## Pull Request Process

1. Update CHANGELOG.md with your changes
2. Ensure all tests pass
3. Update documentation if needed
4. Request review from maintainers

## Areas for Contribution

- Bug fixes
- New features
- Documentation improvements
- Performance optimizations
- Test coverage
- Plugin development

For more details, see the [full contributing guide](docs/contributing/Contributing.md).

