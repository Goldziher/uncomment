# uncomment

[![PyPI version](https://badge.fury.io/py/uncomment.svg)](https://badge.fury.io/py/uncomment)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Goldziher/uncomment/blob/main/LICENSE)

A fast, accurate comment removal tool using tree-sitter for AST parsing. Perfect for cleaning up AI-generated code with excessive comments.

## Installation

```bash
pip install uncomment
```

> **Note**: This Python package downloads and wraps the native Rust binary, providing fast performance without requiring Rust to be installed.

## Quick Start

```bash
# Remove comments from a single file
uncomment src/app.py

# Process entire directory with 8 parallel threads
uncomment --threads 8 src/

# Preview changes without modifying files
uncomment --dry-run src/

# Remove TODO/FIXME comments too
uncomment --remove-todo --remove-fixme src/

# Remove documentation comments
uncomment --remove-doc src/
```

## Supported Languages

- **Python** (.py, .pyw, .pyi, .pyx, .pxd)
- **JavaScript** (.js, .jsx, .mjs, .cjs)
- **TypeScript** (.ts, .tsx, .mts, .cts, .d.ts)
- **Rust** (.rs)
- **Go** (.go)
- **Java** (.java)
- **C/C++** (.c, .h, .cpp, .hpp, .cc, .hh)
- **Ruby** (.rb)
- **YAML** (.yml, .yaml)
- **HCL/Terraform** (.hcl, .tf, .tfvars)
- **Makefile** (Makefile, .mk)

## Features

- **100% Accurate**: Uses tree-sitter AST parsing - never removes comment-like content from strings
- **Smart Preservation**: Automatically keeps important metadata:
  - Linting directives (ESLint, Biome, Pylint, etc.)
  - Type annotations and coverage ignores
  - TODO/FIXME comments (optional)
  - Documentation comments (optional)
- **High Performance**: Multi-threaded processing with up to 3.4x speedup
- **Safe**: Dry-run mode to preview changes before applying
- **Cross-platform**: Works on Windows, macOS, and Linux

## Why Use This?

Perfect for:

- Cleaning up AI-generated code with excessive explanatory comments
- Preparing code for production by removing development comments
- Batch processing large codebases
- Maintaining clean, professional code without manual comment removal

## Documentation

- **Full Documentation**: [https://github.com/Goldziher/uncomment](https://github.com/Goldziher/uncomment)
- **Issues & Support**: [https://github.com/Goldziher/uncomment/issues](https://github.com/Goldziher/uncomment/issues)
- **Changelog**: [https://github.com/Goldziher/uncomment/releases](https://github.com/Goldziher/uncomment/releases)

## Alternative Installation Methods

```bash
# Rust/Cargo
cargo install uncomment

# Node.js/npm
npm install -g uncomment

# Direct download
# See GitHub releases for pre-built binaries
```

## License

MIT - see [LICENSE](https://github.com/Goldziher/uncomment/blob/main/LICENSE) for details.
