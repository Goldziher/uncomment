# Uncomment: Tree-sitter Based Comment Removal Tool

A fast, accurate comment removal tool that uses tree-sitter for parsing, ensuring 100% accuracy in comment identification across multiple programming languages.

## Features

- **100% Accurate**: Uses tree-sitter AST parsing to correctly identify comments
- **No False Positives**: Never removes comment-like content from strings
- **Smart Preservation**: Keeps important metadata, TODOs, FIXMEs, and language-specific patterns
- **Parallel Processing**: Multi-threaded processing for improved performance
- **Extensible**: Easy to add new languages through configuration
- **Fast**: Leverages tree-sitter's optimized parsing
- **Safe**: Dry-run mode to preview changes
- **Built-in Benchmarking**: Performance analysis and profiling tools

## Supported Languages

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
- Zig (.zig)

## Installation

### Via Package Managers

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

## Usage

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

# Benchmark performance on a large codebase
uncomment benchmark --target /path/to/repo --iterations 3

# Profile performance with detailed analysis
uncomment profile /path/to/repo
```

## Default Preservation Rules

### Always Preserved

- Comments containing `~keep`
- TODO comments (unless `--remove-todo`)
- FIXME comments (unless `--remove-fixme`)
- Documentation comments (unless `--remove-doc`)

### Language-Specific Preservation

**Python:**

- Type hints: `# type:`, `# mypy:`
- Linting: `# noqa`, `# pylint:`, `# flake8:`, `# ruff:`
- Formatting: `# fmt:`, `# isort:`
- Other: `# pragma:`, `# NOTE:`

**JavaScript/TypeScript:**

- Type checking: `@flow`, `@ts-ignore`, `@ts-nocheck`
- Linting: `eslint-disable`, `eslint-enable`, `biome-ignore`
- Formatting: `prettier-ignore`
- Coverage: `v8 ignore`, `c8 ignore`, `istanbul ignore`
- Other: `@jsx`, `@license`, `@preserve`

**Rust:**

- Attributes and directives (preserved in comment form)
- Doc comments `///` and `//!` (unless `--remove-doc`)
- Clippy directives: `clippy::`

**Zig:**

- Line Comments: `//`
- Doc comments: `///` (unless `--remove-doc`)
- Top-level Doc comments: `//!`

**Haskell:**

- Comments: `--`
- Haddock: `-- |`, `{-^ ... -}`, `{-| ... -}` (unless `--remove-doc`)

**YAML/HCL/Makefile:**

- Standard comment removal while preserving file structure
- Supports both `#` and `//` style comments in HCL/Terraform

## How It Works

Unlike regex-based tools, uncomment uses tree-sitter to build a proper Abstract Syntax Tree (AST) of your code. This means it understands the difference between:

- Real comments vs comment-like content in strings
- Documentation comments vs regular comments
- Inline comments vs standalone comments
- Language-specific metadata that should be preserved

## Architecture

The tool is built with a generic, extensible architecture:

1. **Language Registry**: Dynamically loads language configurations
2. **AST Visitor**: Traverses the tree-sitter AST to find comments
3. **Preservation Engine**: Applies rules to determine what to keep
4. **Output Generator**: Produces clean code with comments removed

## Adding New Languages

Languages are configured through the registry. To add a new language:

1. Add the tree-sitter parser dependency
2. Register the language in `src/languages/registry.rs`
3. Define comment node types and preservation patterns
4. That's it! No other code changes needed

## Git Hooks

### Pre-commit

Add to your `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/Goldziher/uncomment
    rev: v2.2.0
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

## License

MIT
