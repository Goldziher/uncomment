# Uncomment: Tree-sitter Based Comment Removal Tool

A fast, accurate comment removal tool that uses tree-sitter for parsing, ensuring 100% accuracy in comment identification across multiple programming languages.

## Features

- **100% Accurate**: Uses tree-sitter AST parsing to correctly identify comments
- **No False Positives**: Never removes comment-like content from strings
- **Smart Preservation**: Keeps important metadata, TODOs, FIXMEs, and language-specific patterns
- **Extensible**: Easy to add new languages through configuration
- **Fast**: Leverages tree-sitter's optimized parsing
- **Safe**: Dry-run mode to preview changes

## Supported Languages

- Python (.py, .pyw, .pyi)
- JavaScript (.js, .jsx, .mjs, .cjs)
- TypeScript (.ts, .tsx, .mts, .cts)
- Rust (.rs)
- Go (.go)
- Java (.java)
- C (.c, .h)
- C++ (.cpp, .cc, .cxx, .hpp, .hxx)
- Ruby (.rb, .rake, .gemspec)

## Installation

```bash
cargo install --path .
```

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
- Linting: `eslint-disable`, `eslint-enable`
- Formatting: `prettier-ignore`
- Other: `@jsx`, `@license`, `@preserve`

**Rust:**

- Attributes and directives (preserved in comment form)
- Doc comments `///` and `//!` (unless `--remove-doc`)

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

## Performance

While slightly slower than regex-based approaches due to parsing overhead, the tool is still very fast:

- Small files (<1000 lines): ~20-30ms
- Large files (>10000 lines): ~100-200ms

The accuracy gained is worth the small performance cost.

## License

MIT
