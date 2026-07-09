---
priority: high
---

# Project Architecture

uncomment is a Rust CLI tool for AST-based comment removal from source code, distributed via multiple package ecosystems.

## Source Code (`src/`)

- `main.rs` — Entry point, CLI argument parsing
- `cli.rs` — CLI interface and argument definitions
- `config.rs` — TOML configuration loading and merging
- `processor.rs` — Main file processing logic (orchestrates parsing, detection, removal)
- `ast/` — AST visitor pattern for tree traversal and comment detection
  - `visitor.rs` — Core visitor implementation
- `grammar/` — Tree-sitter grammar loading and management
  - `mod.rs` — Static grammar registry (`static_languages()` HashMap), dynamic loader
- `languages/` — Language definitions and registry
  - `registry.rs` — Language configurations (extensions, comment nodes, preserve patterns)
- `rules/` — Comment preservation rule engine

## Distribution

- `npm-package/` — npm wrapper package (`uncomment-cli`)
  - `package.json`, `install.js` — Binary download and install
- `pip-package/` — PyPI wrapper package (`uncomment`)
  - `pyproject.toml`, `uncomment/__init__.py`, `uncomment/downloader.py`
- `.github/workflows/` — CI/CD and release automation (`publish.yaml` builds cross-platform binaries via a native matrix)
- `scripts/update-homebrew-formula.sh` — Regenerates the Homebrew formula from release checksums

## Testing

- `tests/` — Integration tests
- `fixtures/languages/` — Test source files for each supported language
- Unit tests are co-located in source files using `#[cfg(test)]`

## Configuration Files

- `.uncommentrc.toml` — Per-project configuration
- `~/.config/uncomment/config.toml` — Global user configuration
- `examples/` — Example configuration files and usage

## Build Artifacts

- `target/` — Cargo build output (gitignored)
- `~/.cache/uncomment/grammars/` — Cached compiled grammars (runtime)
