---
type: Concept
title: Development Setup
x-ai-rulez:
  kind: context
  id: development-setup
  metadata:
    priority: high
---

## Development Setup

### Prerequisites

- Rust 1.90+ (`rustc --version`)
- Cargo (included with Rust)
- mise (`mise --version`) for the toolchain and tasks

### Build

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run locally
cargo run -- --help
```

### Test

```bash
# Run all tests
cargo test

# Verbose output
cargo test -- --nocapture

# Run explicitly ignored tests when relevant
cargo test -- --ignored

# Specific test file
cargo test --test integration_tests

# Test on actual files
cargo run -- fixtures/languages/test.py --dry-run
```

### Lint & Format

```bash
# Format code
cargo fmt --all

# Lint with clippy
cargo clippy

# Lint everything with poly
poly lint .

# Check formatting
poly fmt --check .
```

### Benchmarks & Profiling

```bash
# Run benchmarks
cargo bench

# Profile on a real codebase (requires bench-tools feature)
cargo run --release --features bench-tools --bin profile -- /path/to/repo
```

### Task Runner

Use `mise run` for common operations:

```bash
mise tasks ls        # List available tasks
mise run setup       # Install the toolchain, dependencies and hooks
mise run build       # Release build
mise run test        # Run tests
mise run lint        # Run linters
mise run check       # Lint + test
mise run format      # Format code
mise run clean       # Clean build artifacts
```
