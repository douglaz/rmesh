# Git Hooks

This directory contains Git hooks for maintaining code quality in the rmesh project.

## Setup

The hooks are automatically configured when you enter the Nix development shell:

```bash
nix develop
```

Alternatively, you can manually configure them:

```bash
git config core.hooksPath .githooks
```

## Hooks

### pre-commit

Runs before each commit to ensure code is properly formatted:
- ✅ Checks code formatting with `cargo fmt`
- ❌ Blocks commit if code is not formatted
- 💡 Suggests fix: `cargo fmt` or `nix develop -c cargo fmt`

### pre-push

Runs before each push to ensure code quality:
- ✅ Checks code formatting
- ✅ Runs clippy linting with strict warnings
- ✅ Runs all tests
- ⚠️  Warns about outdated dependencies (non-blocking)

## Disabling Hooks

To temporarily skip hooks:
```bash
git commit --no-verify
git push --no-verify
```

To permanently disable:
```bash
git config --unset core.hooksPath
```

## Requirements

- Rust toolchain with `cargo fmt` and `cargo clippy`
- Nix (recommended) for consistent environment
- Optional: `cargo-outdated` for dependency checks