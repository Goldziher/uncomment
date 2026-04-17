---
priority: high
---

# Development Setup

## Prerequisites

- Rust 1.70+ (`rustc --version`)
- Cargo (included with Rust)
- Task runner (`task --version`) for task automation

## Build

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run locally
cargo run -- --help
```

## Test

```bash
# Run all tests
cargo test

# Verbose output
cargo test -- --nocapture

# Network-dependent tests (grammar loading)
cargo test -- --ignored

# Specific test file
cargo test --test integration_tests

# Test on actual files
cargo run -- fixtures/languages/test.py --dry-run
```

## Lint & Format

```bash
# Format code
cargo fmt --all

# Lint with clippy
cargo clippy

# Run all pre-commit hooks
prek run --all-files
```

## Benchmarks & Profiling

```bash
# Run benchmarks
cargo bench

# Profile on a real codebase (requires bench-tools feature)
cargo run --release --features bench-tools --bin profile -- /path/to/repo
```

## Task Runner

Use `task` for common operations:

```bash
task          # List available tasks
task setup    # Install dependencies and hooks
task build    # Release build
task test     # Run tests
task lint     # Run linters
task check    # Lint + test
task format   # Format code
task clean    # Clean build artifacts
```
