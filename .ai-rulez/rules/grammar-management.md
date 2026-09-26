---
priority: high
---

# Grammar Management

Every grammar comes from the `tree-sitter-language-pack` crate, resolved at runtime by
`tree_sitter_language_pack::get_language(&language_config.tslp_name)`. There is no grammar
compilation, no git or local grammar source, no `~/.cache` and no shared-library loading —
so a language uncomment does not already know is a *registration* problem, never a build one.

## Adding a built-in language

1. Add a constructor to `src/languages/config.rs` following the existing ones:
   `LanguageConfig::new(name, extensions, comment_nodes, doc_comment_nodes, tslp_name)`,
   chained with `.with_comment_syntax(...)`.
2. Add it to the `configs` vector in `register_default_languages` in
   `src/languages/registry.rs`.
3. Add a fixture under `fixtures/languages/`.

`comment_nodes` and `doc_comment_nodes` are tree-sitter **node kinds**, not literal
delimiters — `line_comment`, `block_comment`, `comment`, `string`. Inspect the real parse
tree rather than guessing; the kinds differ per grammar, and Python records a docstring as
`string`. `tslp_name` is the language-pack key, which is not always the language name.

`comment_syntax` is the separate, literal delimiter data (`CommentSyntax { line, block }`),
used to write `~keep` markers rather than to find comments. Its `line` field is **always
the plain form** (`//`, `#`, `--`), never a doc form such as `///` or `##`: a marker written
with a doc prefix is classified as documentation and silently does nothing. Use `None` when
a language has no line-comment form.

## Guidelines

- A missing or misnamed `tslp_name` fails at parse time, per file. Report it with the
  language name and the key that was tried, as `Processor` does.
- `register_configured_languages` lets a user's config override a built-in; it inherits
  `tslp_name` and `comment_syntax` from the built-in being overridden, so an override that
  only changes `comment_nodes` keeps working.
