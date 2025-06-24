# uncomment

A fast, accurate comment removal tool using tree-sitter for AST parsing.

## Installation

```bash
npm install -g uncomment
```

## Usage

```bash
# Remove comments from a file
uncomment file.py

# Process with parallel threads
uncomment --threads 8 src/

# Dry run to preview changes
uncomment --dry-run file.js
```

## Features

- **100% Accurate**: Uses tree-sitter AST parsing
- **Multi-language support**: Python, JavaScript, TypeScript, Rust, Go, Java, C/C++, Ruby, YAML, HCL/Terraform, Makefile
- **Smart preservation**: Keeps linting directives, TODOs, and documentation
- **Parallel processing**: Multi-threaded for performance
- **Safe**: Dry-run mode available

## Documentation

For full documentation, visit: https://github.com/Goldziher/uncomment

## License

MIT
