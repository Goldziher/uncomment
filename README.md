# Uncomment: Tree-sitter Based Comment Removal Tool

A fast, accurate, and extensible comment removal tool that uses tree-sitter for parsing, ensuring 100% accuracy in comment identification. Originally created to clean up AI-generated code with excessive comments, it now supports any language with a tree-sitter grammar through its flexible configuration system.

## Support This Project

If you find uncomment helpful, please consider sponsoring the development:

<a href="https://github.com/sponsors/Goldziher"><img src="https://img.shields.io/badge/Sponsor-%E2%9D%A4-pink?logo=github-sponsors" alt="Sponsor on GitHub" height="32"></a>

Your support helps maintain and improve this tool for the community! 🚀

## Features

- **100% Accurate**: Uses tree-sitter AST parsing to correctly identify comments
- **No False Positives**: Never removes comment-like content from strings
- **Smart Preservation**: Keeps important metadata, TODOs, FIXMEs, and language-specific patterns
- **Parallel Processing**: Multi-threaded processing for improved performance
- **Extensible**: Support any language with tree-sitter grammar through configuration
- **Dynamic Grammar Loading**: Load grammars from Git, local paths, or pre-compiled libraries
- **Configuration System**: TOML-based configuration for project-specific settings
- **Smart Init Command**: Automatically generate configuration based on your project
- **Fast**: Leverages tree-sitter's optimized parsing
- **Safe**: Dry-run mode to preview changes
- **Built-in Benchmarking**: Performance analysis and profiling tools

## Supported Languages

### Built-in Languages

- Python (.py, .pyw, .pyi, .pyx, .pxd)
- JavaScript (.js, .jsx, .mjs, .cjs)
- TypeScript (.ts, .tsx, .mts, .cts, .d.ts, .d.mts, .d.cts)
- Rust (.rs)
- Go (.go)
- Java (.java)
- C (.c, .h)
- C++ (.cpp, .cc, .cxx, .hpp, .hxx)
- Ruby (.rb, .rake, .gemspec)
- YAML (.yml, .yaml)
- HCL/Terraform (.hcl, .tf, .tfvars)
- Makefile (Makefile, .mk)
- Shell/Bash (.sh, .bash, .zsh, .bashrc, .zshrc)
- Haskell (.hs, .lhs)
- JSON with Comments (.jsonc)

### Extensible to Any Language

Through the configuration system, you can add support for any language with a tree-sitter grammar, including:

- Vue, Svelte, Astro (Web frameworks)
- Swift, Kotlin, Dart (Mobile development)
- Zig, Nim (Systems programming)
- Elixir, Clojure, Julia (Functional/Scientific)
- And many more...

## Installation

### Via Package Managers

#### Homebrew (macOS/Linux)

```bash
brew tap goldziher/tap
brew install uncomment
```

#### Cargo (Rust)

```bash
cargo install uncomment
```

#### npm (Node.js)

```bash
npm install -g uncomment-cli
```

#### pip (Python)

```bash
pip install uncomment
```

### From source

```bash
git clone https://github.com/Goldziher/uncomment.git
cd uncomment
cargo install --path .
```

### Requirements

- For building from source: Rust 1.70+
- For npm/pip packages: Pre-built binaries are downloaded automatically

## Quick Start

### Run Without Installing

```bash
npx -y uncomment-cli@latest .
uvx uncomment .
```

Add `--dry-run` to either command to preview changes before writing.

### Install Locally

```bash
# Generate a configuration file for your project
uncomment init

# Remove comments from files
uncomment src/

# Preview changes without modifying files
uncomment --dry-run src/
```

## Usage

### Configuration

```bash
# Generate a smart configuration based on your project
uncomment init

# Generate a comprehensive configuration with all supported languages
uncomment init --comprehensive

# Interactive configuration setup
uncomment init --interactive

# Use a custom configuration file
uncomment --config my-config.toml src/
```

#### Init Command Examples

The `init` command intelligently detects languages in your project:

```bash
# Smart detection - analyzes your project and includes only detected languages
$ uncomment init
Detected languages in your project:
- 150 rust files
- 89 typescript files
- 45 python files
- 12 vue files (requires custom grammar)
- 8 dockerfile files (requires custom grammar)

Generated .uncommentrc.toml with configurations for detected languages.

# Comprehensive mode - includes configurations for 25+ languages
$ uncomment init --comprehensive
Generated comprehensive configuration with all supported languages.

# Specify output location
$ uncomment init --output config/uncomment.toml

# Force overwrite existing configuration
$ uncomment init --force
```

### Basic Usage

```bash
# Remove comments from a single file
uncomment file.py

# Preview changes without modifying files
uncomment --dry-run file.py

# Process multiple files
uncomment src/*.py

# Remove documentation comments/docstrings
uncomment --remove-doc file.py

# Remove TODO and FIXME comments
uncomment --remove-todo --remove-fixme file.py

# Add custom patterns to preserve
uncomment --ignore-patterns "HACK" --ignore-patterns "WARNING" file.py

# Process entire directory recursively
uncomment src/

# Use parallel processing with 8 threads
uncomment --threads 8 src/
```

### Optional Benchmarking Tools

The crate ships development binaries for benchmarking and profiling, but they are gated behind the `bench-tools` feature so they are not installed for regular users.

- Install from crates.io with the extras:
  ```bash
  cargo install uncomment --features bench-tools
  ```
- Run locally without installing:
  ```bash
  cargo run --release --features bench-tools --bin benchmark -- --target /path/to/repo --iterations 3
  cargo run --release --features bench-tools --bin profile -- /path/to/repo
  ```

## Contributing

See `CONTRIBUTING.md` for local development, automation hooks, and release procedures.

## Default Preservation Rules

### Always Preserved

- Comments containing `~keep`
- TODO comments (unless `--remove-todo`)
- FIXME comments (unless `--remove-fixme`)
- Documentation comments (unless `--remove-doc`)

### Linting Tool Directives (Always Preserved)

The tool preserves all linting and formatting directives to ensure your CI/CD pipelines and development workflows remain intact:

**Go:**

- `//nolint`, `//nolint:gosec`, `//golangci-lint`, `//staticcheck`, `//go:generate`

**Python:**

- `# noqa`, `# type: ignore`, `# mypy:`, `# pyright:`, `# ruff:`, `# pylint:`, `# flake8:`
- `# fmt: off/on`, `# black:`, `# isort:`, `# bandit:`, `# pyre-ignore`

**JavaScript/TypeScript:**

- ESLint: `eslint-disable`, `eslint-enable`, `eslint-disable-next-line`
- TypeScript: `@ts-ignore`, `@ts-expect-error`, `@ts-nocheck`, `@ts-check`
- Triple-slash: `/// <reference`, `/// <amd-module`, `/// <amd-dependency`
- Formatters: `prettier-ignore`, `biome-ignore`, `deno-lint-ignore`
- Coverage: `v8 ignore`, `c8 ignore`, `istanbul ignore`

**Rust:**

- Attributes: `#[allow]`, `#[deny]`, `#[warn]`, `#[forbid]`, `#[cfg]`
- Clippy: `clippy::`, `#[rustfmt::skip]`

**Java:**

- `@SuppressWarnings`, `@SuppressFBWarnings`, `//noinspection`, `// checkstyle:`

**C/C++:**

- `// NOLINT`, `// NOLINTNEXTLINE`, `#pragma`, `// clang-format off/on`

**Shell/Bash:**

- `# shellcheck disable`, `# hadolint ignore`

**YAML:**

- `# yamllint disable/enable`

**HCL/Terraform:**

- `# tfsec:ignore`, `# checkov:skip`, `# trivy:ignore`, `# tflint-ignore`

**Ruby:**

- `# rubocop:disable/enable`, `# reek:`, `# standard:disable/enable`

## Configuration

Uncomment uses a flexible TOML-based configuration system that allows you to customize behavior for your project.

### Configuration File Discovery

Uncomment searches for configuration files in the following order:

1. Command-line specified config: `--config path/to/config.toml`
2. `.uncommentrc.toml` in the current directory
3. `.uncommentrc.toml` in parent directories (up to git root or filesystem root)
4. `~/.config/uncomment/config.toml` (global configuration)
5. Built-in defaults

### Basic Configuration Example

```toml
[global]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = ["IMPORTANT", "NOTE", "WARNING"]
use_default_ignores = true
respect_gitignore = true

[languages.python]
extensions = ["py", "pyw", "pyi"]
preserve_patterns = ["noqa", "type:", "pragma:", "pylint:"]

[patterns."tests/**/*.py"]
# Keep all comments in test files
remove_todos = false
remove_fixme = false
remove_docs = false
```

### Dynamic Grammar Loading

You can extend support to any language with a tree-sitter grammar:

```toml
# Add Swift support via Git
[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }

# Use a local grammar
[languages.custom]
name = "Custom Language"
extensions = ["custom"]
comment_nodes = ["comment"]

[languages.custom.grammar]
source = { type = "local", path = "/path/to/tree-sitter-custom" }

# Use a pre-compiled library
[languages.proprietary]
name = "Proprietary Language"
extensions = ["prop"]
comment_nodes = ["comment"]

[languages.proprietary.grammar]
source = { type = "library", path = "/usr/local/lib/libtree-sitter-proprietary.so" }
```

### Configuration Merging

When multiple configuration files are found, they are merged with the following precedence (highest to lowest):

1. Command-line flags
2. Local `.uncommentrc.toml` files (closer to the file being processed wins)
3. Global configuration (`~/.config/uncomment/config.toml`)
4. Built-in defaults

Pattern-specific configurations override language configurations for matching files.

## How It Works

Unlike regex-based tools, uncomment uses tree-sitter to build a proper Abstract Syntax Tree (AST) of your code. This means it understands the difference between:

- Real comments vs comment-like content in strings
- Documentation comments vs regular comments
- Inline comments vs standalone comments
- Language-specific metadata that should be preserved

## Architecture

The tool is built with a modular, extensible architecture:

1. **Language Registry**: Manages both built-in and dynamically loaded languages
2. **Grammar Manager**: Handles loading grammars from Git, local paths, or compiled libraries
3. **Configuration System**: TOML-based hierarchical configuration with merging
4. **AST Visitor**: Traverses the tree-sitter AST to find comments
5. **Preservation Engine**: Applies rules to determine what to keep
6. **Output Generator**: Produces clean code with comments removed

### Key Components

- **Dynamic Grammar Loading**: Automatically downloads and compiles tree-sitter grammars
- **Grammar Caching**: Caches compiled grammars for performance
- **Configuration Discovery**: Searches for configs in project hierarchy
- **Pattern Matching**: File-pattern-specific configuration overrides

## Adding New Languages

With the new configuration system, you can add languages without modifying code:

### Method 1: Using Configuration (Recommended)

Add to your `.uncommentrc.toml`:

```toml
[languages.mylang]
name = "My Language"
extensions = ["ml", "mli"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO", "FIXME"]

[languages.mylang.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-mylang", branch = "main" }
```

### Method 2: Built-in Support

For frequently used languages:

1. Add the tree-sitter parser dependency to `Cargo.toml`
2. Register the language in `src/grammar/mod.rs`
3. Add language configuration in `src/languages/registry.rs`

## Git Hooks

### Pre-commit

Add to your `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/Goldziher/uncomment
    rev: v2.5.0
    hooks:
      - id: uncomment
```

### Lefthook

Add to your `lefthook.yml`:

```yaml
pre-commit:
  commands:
    uncomment:
      run: uncomment {staged_files}
      stage_fixed: true
```

For both hooks, install uncomment via pip:

```bash
pip install uncomment
```

## Performance

While slightly slower than regex-based approaches due to parsing overhead, the tool is very fast and scales well with parallel processing:

### Single-threaded Performance

- Small files (<1000 lines): ~20-30ms
- Large files (>10000 lines): ~100-200ms

### Parallel Processing Benchmarks

Performance scales excellently with multiple threads:

| Thread Count | Files/Second | Speedup |
| ------------ | ------------ | ------- |
| 1 thread     | 1,500        | 1.0x    |
| 4 threads    | 3,900        | 2.6x    |
| 8 threads    | 5,100        | 3.4x    |

_Benchmarks run on a large enterprise codebase with 5,000 mixed language files_

### Built-in Benchmarking

Use the built-in tools to measure performance on your specific codebase:

```bash
# Basic benchmark
uncomment benchmark --target /path/to/repo

# Detailed benchmark with multiple iterations
uncomment benchmark --target /path/to/repo --iterations 5 --threads 8

# Memory and performance profiling
uncomment profile /path/to/repo
```

The accuracy gained through AST parsing is worth the small performance cost, and parallel processing makes it suitable for even the largest codebases.

## Development

### Project Structure

```
uncomment/
├── src/               # Source code
├── tests/             # Integration tests
├── fixtures/          # Test fixtures
│   ├── languages/     # Language-specific test files
│   └── repos/         # Repository test configurations
├── bench/             # CLI benchmarking tool
├── benchmarks/        # Rust micro-benchmarks
├── test-repos/        # Manual testing scripts
└── scripts/           # Build and release scripts
```

### Benchmarking

The project includes two types of benchmarking tools:

- **`bench/`**: Custom CLI benchmarking tool for testing real-world performance on large codebases. Use via `cargo run --release --bin benchmark`.
- **`benchmarks/`**: Standard Rust micro-benchmarks for testing specific functions and components. Run with `cargo bench`.

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run integration tests (including network-dependent)
cargo test -- --ignored
```

## License

MIT
