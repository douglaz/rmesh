# Contributing to rmesh

Thank you for your interest in contributing to rmesh! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Please be respectful and constructive in all interactions. We welcome contributors of all experience levels.

## Getting Started

### Prerequisites

- Rust 1.70 or higher
- Git
- (Optional) NixOS/Nix for development environment
- (Optional) A Meshtastic device for testing

### Development Setup

1. Fork the repository on GitHub
2. Clone your fork:
```bash
git clone https://github.com/yourusername/rmesh.git
cd rmesh
```

3. Enter the development environment:
```bash
# Using Nix (recommended)
nix develop

# Or use standard Rust toolchain
cargo build
```

## Development Workflow

### Branch Naming

Create descriptive branch names:
- `feature/` - New features
- `fix/` - Bug fixes  
- `refactor/` - Code refactoring
- `docs/` - Documentation updates
- `test/` - Test improvements
- `chore/` - Maintenance tasks

### Making Changes

1. Create a new branch:
```bash
git checkout -b feature/your-feature-name
```

2. Make your changes following the coding standards below

3. Run tests:
```bash
cargo test
```

4. Check code quality:
```bash
cargo clippy
cargo fmt --check
```

5. Commit your changes:
```bash
git add .
git commit -m "feat: add new feature description"
```

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `refactor:` - Code refactoring
- `test:` - Test additions/modifications
- `chore:` - Maintenance tasks
- `perf:` - Performance improvements

Examples:
```
feat(decoder): add support for new packet types
fix(connection): handle timeout correctly
docs(readme): update installation instructions
```

## Coding Standards

### Rust Style

- Follow standard Rust naming conventions
- Use `cargo fmt` to format code
- Fix all `cargo clippy` warnings
- Add documentation comments for public APIs
- Write unit tests for new functionality

### Error Handling

- Use `anyhow::Result` for error propagation
- Add context to errors with `.context()`
- Use `bail!` for early returns with errors
- Provide helpful error messages

### Testing

- Write unit tests for new functionality
- Place tests in the same file as the code being tested
- Use descriptive test names
- Test both success and failure cases

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_success() -> Result<()> {
        // Test implementation
        Ok(())
    }

    #[test]
    fn test_feature_error_case() -> Result<()> {
        // Test error handling
        Ok(())
    }
}
```

## Project Structure

```
rmesh/
├── rmesh-core/    # Core library
│   ├── src/
│   │   ├── lib.rs          # Public API
│   │   ├── connection/     # Connection management
│   │   ├── state.rs        # Device state
│   │   └── ...            # Feature modules
│   └── Cargo.toml
├── rmesh/         # CLI binary
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── cli.rs          # CLI definitions
│   │   ├── commands/       # Command implementations
│   │   └── output/         # Output formatting
│   └── Cargo.toml
├── rmesh-test/          # Hardware testing tool
└── Cargo.toml             # Workspace definition
```

## Adding New Features

### Adding a New Command

1. Define the command in `rmesh/src/cli.rs`
2. Implement the handler in `rmesh/src/commands/`
3. Add core logic in `rmesh-core/src/`
4. Update documentation in README.md
5. Add tests

### Adding Protocol Support

1. Update protobuf definitions if needed
2. Implement packet handling in `connection/manager.rs`
3. Add state management in `state.rs`
4. Create high-level API in appropriate module
5. Add CLI command to expose functionality

## Testing

### Unit Tests
```bash
cargo test
```

### Integration Tests with Device
```bash
# Run hardware test suite
cargo run --bin rmesh-test -- --port /dev/ttyACM0
```

### Manual Testing
```bash
# Build and test locally
cargo build
./target/debug/rmesh --port /dev/ttyACM0 info radio
```

## Documentation

- Update README.md for user-facing changes
- Add inline documentation for public APIs
- Update CHANGELOG.md for notable changes
- Keep examples current and working

## Submitting Changes

1. Push your branch to your fork:
```bash
git push origin feature/your-feature-name
```

2. Create a Pull Request on GitHub
3. Provide a clear description of changes
4. Link any related issues
5. Wait for review and address feedback

## Pull Request Checklist

Before submitting a PR, ensure:

- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Documentation is updated
- [ ] Commit messages follow convention
- [ ] Branch is up to date with main

## Getting Help

- Open an issue for bugs or feature requests
- Join the Meshtastic Discord for discussions
- Check existing issues and PRs before starting work

## License

By contributing, you agree that your contributions will be dual-licensed under MIT OR Apache-2.0.