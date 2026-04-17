# Uncomment Configuration Examples

This directory contains example configuration files for `uncomment`.

## Files

### `builtin_languages.toml`

Comprehensive example showing how to configure all 49 built-in languages with their specific settings and preservation patterns.

### `custom_languages.toml`

Example demonstrating how to add languages beyond the 49 built-ins. Any of the 306 languages supported by [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack) can be added with just a name, extensions, and comment node types — grammars are downloaded automatically.

### `quickstart_custom_languages.toml`

Minimal example for quickly customizing language settings.

## Adding Languages

To customize or add a language, create an `.uncommentrc.toml` in your project root:

```toml
[languages.hare]
name = "Hare"
extensions = ["ha"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO", "FIXME"]
```

No grammar configuration is needed — uncomment uses [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack) which automatically downloads grammars on first use.

## Built-in Languages (49)

Rust, Python, JavaScript, TypeScript, TSX, Go, Ruby, PHP, Elixir, TOML, C#, Java, C, C++, JSON, JSONC, YAML, HCL/Terraform, Makefile, Shell/Bash, Haskell, HTML, CSS, XML, SQL, Kotlin, Swift, Lua, Nix, PowerShell, Protobuf, INI, Dockerfile, Scala, Dart, R, Julia, Zig, Clojure, Elm, Erlang, Vue, Svelte, SCSS, LaTeX, Fish, Perl, Groovy, OCaml, Fortran
