# Uncomment Configuration Examples

This directory contains example configuration files for `uncomment`.

## Files

### `builtin_languages.toml`

Comprehensive example showing how to configure all builtin languages with their specific settings and preservation patterns.

### `custom_languages.toml`

Extensive example demonstrating how to add languages that are not included as builtins, using various grammar sources (Git, local, library).

### `quickstart_custom_languages.toml`

Minimal example for quickly adding custom language support for Ruby, Vue.js, and Swift.

## Adding Custom Languages

To add a language that's not included in the builtins:

1. Create an `uncomment.toml` file in your project root
2. Define the language with its grammar source:

```toml
[languages.vue]
name = "Vue"
extensions = ["vue"]
comment_nodes = ["comment"]

[languages.vue.grammar]
source = { type = "git", url = "https://github.com/tree-sitter-grammars/tree-sitter-vue", branch = "main" }
```

## Grammar Sources

### Git (Recommended)

Downloads and compiles grammar from a Git repository:

```toml
[languages.ruby.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-ruby", branch = "master" }
```

### Local Directory

Uses a grammar from a local directory:

```toml
[languages.custom.grammar]
source = { type = "local", path = "/path/to/grammar" }
```

### Pre-compiled Library

Uses a pre-compiled grammar library:

```toml
[languages.proprietary.grammar]
source = { type = "library", path = "/usr/local/lib/libtree-sitter-lang.so" }
```

## Builtin Languages

The following languages are included as builtins and don't require custom grammar configuration:

- Rust
- Python
- JavaScript
- TypeScript (including TSX)
- Go
- Java
- C
- C++
- JSON (and JSONC)
- YAML
- HCL/Terraform
- Makefile
- Shell/Bash

## Finding Tree-Sitter Grammars

Most languages have tree-sitter grammars available. Search for `tree-sitter-[language]` on GitHub. Popular grammar repositories:

- https://github.com/tree-sitter
- https://github.com/tree-sitter-grammars

## Caching

Downloaded and compiled grammars are cached in `~/.cache/uncomment/grammars/` to avoid re-downloading.
