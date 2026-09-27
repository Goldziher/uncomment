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
with a doc prefix is classified as documentation and silently does nothing. `block` is `None`
unless the pair can be written at a comment's own indentation — Ruby's `=begin`/`=end` and Perl's
POD are recognised only at column 0, so they are not offered. Use `None` for a form the language
does not have; a language with a `block` pair and no `line` token is still markable, because the
marker is written with the pair.

## Guidelines

- A misnamed `tslp_name` on a **built-in** fails at parse time, per file. Report it with the
  language name and the key that was tried, as `Processor` does.
- A **config-declared** language whose `name` matches neither a built-in nor a pack grammar
  registers nothing at all — not even its extensions, so the files it was written for are
  never collected. `register_configured_languages` returns those names and
  `warn_languages_without_a_grammar` reports them once per run; never drop one silently,
  because the only symptom is a file that quietly goes unprocessed.
- `register_configured_languages` lets a user's config override a built-in; it inherits
  `tslp_name` and `comment_syntax` from the built-in being overridden, so an override that
  only changes `comment_nodes` keeps working. The lookup is on `name` lowercased, not on the
  `[languages.<key>]` section key, which is arbitrary.
