---
priority: high
---

# Comment Preservation Rules

uncomment supports selective comment preservation. Certain comments must not be removed even during full comment stripping.

## Preserved Comment Types

- **TODO/FIXME markers**: Comments containing `TODO`, `FIXME`, `HACK`, `XXX`, or `NOSONAR`
- **Linting directives**: Language-specific linting control comments (e.g., `eslint-disable`, `noqa`, `rubocop:disable`, `nolint`, `@suppress`, `swiftlint:`)
- **Keep marker**: Comments containing `~keep` are explicitly marked for preservation
- **Docstrings**: Language documentation comments (e.g., `///` in Rust, `"""` in Python, `/** */` in Java/TypeScript)

## Configuration

- Preserve patterns are defined per-language in `src/languages/registry.rs` via the `preserve_patterns` field
- Users can add custom preserve patterns in `.uncommentrc.toml` under `[languages.<name>].preserve_patterns`
- The preservation rules engine lives in `src/rules/`

## Guidelines

- When adding a new language, always define appropriate `preserve_patterns` for that language's linting tools
- Test preservation with edge cases: nested comments, comments at start/end of file, comments on the same line as code
- The `~keep` marker should work universally across all languages
