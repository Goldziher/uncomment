---
priority: high
---

# Project Architecture

uncomment is a Rust CLI tool for AST-based comment removal from source code, distributed via multiple package ecosystems.

## Source Code (`src/`)

- `main.rs` — Entry point, subcommand dispatch
- `cli.rs` — CLI interface and argument definitions
- `lib.rs` — Library surface (`uncomment::…`) the binary and the integration tests share
- `config/` — TOML configuration
  - `file.rs` — the deserialized shapes (`Config`, `GlobalConfig`, `LanguageConfig`, `PatternConfig`)
  - `manager.rs` — `ConfigManager`: discovery, caching, per-file layering
  - `templates.rs` — the `init` template generators
- `processor/` — Main file processing logic (orchestrates parsing, detection, removal)
- `ast/visitor.rs` — AST visitor for tree traversal and comment detection
- `languages/` — Language definitions and registry
  - `config.rs` — `LanguageConfig`: extensions, comment node kinds, line-comment token
  - `registry.rs` — the built-in language configurations
  - `handlers.rs` — language-specific comment handling (Python docstrings, …)
- `rules/preservation.rs` — Comment preservation rule engine
- `scan/` — `uncomment scan`: the comment inventory and its stable ids (`id.rs`)
- `keep.rs` — `uncomment keep`: writing `~keep` markers back from a scan decision
- `check.rs` — `--check`: the removal run as a gate, its exit codes and its report
- `lint/` — `uncomment lint`: tag rules, their config, and the findings they produce
- `edit.rs` — Applying many byte-range edits to one file in a single pass
- `git.rs` — Reading the current branch and the issue key embedded in it
- `changes.rs` — `--changed-only`/`--base`: the files a branch changed, for `lint` and `--check`
- `paths.rs` — Lexical path helpers shared by config resolution and the inventory commands
- `ui.rs` — Terminal presentation: colors, symbols, structured output

Tree-sitter grammars are compiled into the binary by `tree-sitter-language-pack`; there is no
grammar loader and nothing is fetched at runtime.

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

- `.uncomment.toml` — Per-project configuration
- `~/.config/uncomment/config.toml` — Global user configuration
- `examples/` — Example configuration files and usage

## Build Artifacts

- `target/` — Cargo build output (gitignored)
- `~/.cache/uncomment/grammars/` — Cached compiled grammars (runtime)
