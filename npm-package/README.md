# uncomment-cli

[![npm version](https://badge.fury.io/js/uncomment-cli.svg)](https://badge.fury.io/js/uncomment-cli)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Goldziher/uncomment/blob/main/LICENSE)

A blazing fast Rust-based command-line tool that removes comments from your source code. Perfect for cleaning up AI-generated code that comes with excessive explanations.

## Why Use This?

- 🚀 **Lightning Fast** - Built in Rust for maximum performance
- 🎯 **100% Accurate** - Never accidentally removes code that looks like comments
- 🛡️ **Safe by Default** - Preview changes before applying them
- 🌍 **Multi-language** - Supports 12+ programming languages
- 🔧 **Zero Dependencies** - Downloads a self-contained binary

## Installation

```bash
npm install -g uncomment-cli
```

The installer will automatically download the appropriate pre-compiled Rust binary for your platform (Windows, macOS, or Linux).

## Quick Start

Remove comments from a single file:

```bash
uncomment main.py
```

Preview changes without modifying files:

```bash
uncomment --dry-run src/
```

Process all Python files in a directory:

```bash
uncomment "src/**/*.py"
```

## Key Features

### Smart Comment Detection

Unlike simple regex-based tools, `uncomment` understands your code's structure:

```python
# This comment will be removed
code = "# But this won't - it's in a string!"
```

### Preserves Important Comments

Keeps what matters:

- `TODO` and `FIXME` comments (configurable)
- License headers and copyright notices
- Linting directives (`eslint-disable`, `@ts-ignore`, etc.)
- Documentation strings and JSDoc comments (configurable)

### Supported Languages

Python, JavaScript, TypeScript, Rust, Go, Java, C/C++, Ruby, YAML, Terraform/HCL, Makefile, and more!

## Common Use Cases

**Clean up AI-generated code:**

```bash
uncomment generated_code.js
```

**Remove all comments including TODOs:**

```bash
uncomment --remove-todo --remove-fixme src/
```

**Remove documentation comments:**

```bash
uncomment --remove-doc api.ts
```

**Use multiple threads for large codebases:**

```bash
uncomment --threads 8 entire_project/
```

## Documentation

For detailed documentation, advanced options, and examples, visit:
[https://github.com/Goldziher/uncomment](https://github.com/Goldziher/uncomment)

## License

MIT - see [LICENSE](https://github.com/Goldziher/uncomment/blob/main/LICENSE) for details.
