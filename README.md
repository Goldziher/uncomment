# Uncomment

`uncomment` is a fast, efficient CLI tool to remove comments from your source code files.
It was created to solve the common problem of AI assistants adding excessive comments to generated code.

## Features

- Removes comments while preserving important metadata and directives
- Supports multiple programming languages with language-specific awareness
- Preserves code structure and whitespace
- Handles complex cases like comments in strings and embedded comments
- Language-specific smart defaults for preserving important comment patterns
- Fast operation with glob pattern support for batch processing

## Supported Languages

- Rust (`.rs`)
- C (`.c`, `.h`)
- C++ (`.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`)
- Java (`.java`)
- JavaScript (`.js`)
- TypeScript (`.ts`)
- Python (`.py`)
- Ruby (`.rb`)
- Go (`.go`)
- Swift (`.swift`)

## Installation

```shell
cargo install uncomment
```

### From GitHub Releases

You can also download pre-built binaries directly from the [GitHub Releases page](https://github.com/Goldziher/uncomment/releases).

### Using as a Pre-commit Hook

To use `uncomment` as a pre-commit hook:

Add this to your `.pre-commit-config.yaml`:

```yaml
- repo: https://github.com/Goldziher/uncomment
  rev: v1.0.4 # Use the latest version
  hooks:
    - id: uncomment
      # Optional: Add any arguments you want
      # args: [--remove-todo, --remove-fixme]
```

Note - the pre-commit hook relies on the rust toolchain being available on the system.

## Usage

Basic usage:

```shell
uncomment path/to/file.rs             # Process a single file
uncomment src                         # Process all files in a directory (recursively)
uncomment src/*.rs                    # Process multiple files with a glob pattern
uncomment src/**/*.{js,ts}            # Process JavaScript and TypeScript files recursively
```

When given a directory path without glob patterns (like `uncomment src`), the tool automatically expands it to a recursive pattern (`src/**/*`) to process all files within that directory tree.

### Command Line Options

```shell
Usage: uncomment [OPTIONS] <PATHS>...

Arguments:
  <PATHS>...  The file(s) to uncomment - can be file paths or glob patterns

Options:
  -r, --remove-todo          Whether to remove TODO comments [default: false]
  -f, --remove-fixme         Whether to remove FIXME comments [default: false]
  -d, --remove-doc           Whether to remove doc strings [default: false]
  -i, --ignore-patterns <IGNORE_PATTERNS>
                             Comment patterns to keep, e.g. "noqa:", "eslint-disable*", etc.
      --no-default-ignores   Disable language-specific default ignore patterns [default: false]
  -n, --dry-run              Dry run (don't modify files, just show what would be changed) [default: false]
  -h, --help                 Print help
  -V, --version              Print version
```

## Smart Comment Preservation

By default, `uncomment` preserves:

1. Comments containing `~keep` are marked for preservation
2. TODO comments (unless `--remove-todo` is specified)
3. FIXME comments (unless `--remove-fixme` is specified)
4. Documentation comments/docstrings (unless `--remove-doc` is specified)
5. Language-specific patterns (unless `--no-default-ignores` is specified), including:
   - Rust: `#[`, `allow(`, `cfg_attr`, etc.
   - Python: `# noqa`, `# type:`, `# pylint:`, etc.
   - JavaScript/TypeScript: `@flow`, `@ts-ignore`, `eslint-disable`, etc.
   - And many more for each supported language

Additional patterns can be preserved with the `-i/--ignore-patterns` option.

## Examples

Remove all comments except TODOs, FIXMEs, and docstrings:

```shell
uncomment src/*.rs
```

Remove everything including doc strings, TODOs and FIXMEs:

```shell
uncomment src/*.py --remove-todo --remove-fixme --remove-doc
```

Keep comments containing certain patterns:

```shell
uncomment src/*.py -i "some-pattern"
```

Dry run (see what would change without modifying files):

```shell
uncomment src/*.js --dry-run
```

Disable language-specific default ignore patterns:

```shell
uncomment src/*.rs --no-default-ignores
```

## License

[MIT License](LICENSE)
