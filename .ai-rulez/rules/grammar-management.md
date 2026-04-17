---
priority: high
---

# Grammar Management

Tree-sitter grammars are the foundation of uncomment's parsing capabilities. They can be loaded statically (compiled in) or dynamically (at runtime).

## Static Grammars

- Registered in `src/grammar/mod.rs` via the `static_languages()` HashMap
- Added as dependencies in `Cargo.toml` (e.g., `tree-sitter-python = "0.X"`)
- Configured in `src/languages/registry.rs` with `GrammarSource::Static`

## Dynamic Grammar Loading

Three source types are supported:

1. **Git**: `{ type = "git", url = "...", branch = "main" }` — Clones the repository and compiles the grammar
2. **Local**: `{ type = "local", path = "/path/to/grammar" }` — Uses a local grammar directory
3. **Library**: `{ type = "library", path = "/path/to/libtree-sitter-lang.so" }` — Loads a pre-compiled shared library (`.so`/`.dylib`)

## Caching

- Compiled grammars are cached at `~/.cache/uncomment/grammars/`
- Cache invalidation should be handled when grammar source URLs or branches change
- Never block on network requests without a timeout

## Guidelines

- When adding a new built-in language, add the tree-sitter dependency, register it in `grammar/mod.rs`, configure it in `languages/registry.rs`, and add a test fixture in `fixtures/languages/`
- Handle compilation failures gracefully with clear error messages
- Cache language parsers in `GrammarManager` to avoid reinitialization
