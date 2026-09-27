---
priority: high
---

# Comment Preservation Rules

Which comments survive a run is decided in `src/rules/preservation.rs`; the rule set for a file is
assembled in `Processor::create_preservation_rules_from_config` (`src/processor/mod.rs`).

## The effective default set is `comprehensive_rules()`

`use_default_ignores` defaults to `true`, and that is what pulls in `comprehensive_rules()` — so
the linter directives people assume are covered (`rubocop:`, `eslint-disable`, `noqa`,
`type: ignore`, `fmt: off`, `NOSONAR`, …) are on out of the box. `default_rules()` is only the
base `comprehensive_rules()` extends; nothing calls it on its own. Setting
`use_default_ignores = false` drops the whole set, keeping only the `remove_*` flags and the
user's own `preserve_patterns`.

`preserve_patterns` exists only on the *config-side* `LanguageConfig` (`src/config/file.rs`) — it is
user data. Built-in languages carry no patterns of their own; the generated `init` templates write
per-language suggestions (`swiftlint:`, `frozen_string_literal:`) into the user's config instead.

## `~keep` is a guarded marker, not a substring

A comment is not preserved merely because its text contains `~keep`:

- `is_marker_occurrence` rejects the word inside backticks or quotes, and rejects `~keepsake` —
  prose *about* the marker does not become one.
- `is_keep_marker_line` (the standalone marker that protects a neighbour) additionally requires the
  comment to be standalone and **not** a documentation comment. `/// ~keep` and `## ~keep` are
  documentation, so they are inert — write the plain form (`// ~keep`, `# ~keep`, `<!-- ~keep -->`).
- `redundant_keep_markers` excises a bare `~keep` from inside a doc comment when `remove_docs` is
  false, because the comment was already going to be kept. Never append a marker in-body to a doc
  comment; put it on the line above.

## Guidelines

- Adding a language means no preservation work — the rule set is language-independent.
- A `Pattern` rule is `content.contains(pattern)`: case-sensitive, matching anywhere in the comment,
  never a regex. That is why `TODO` and `todo` are registered as two rules, and why a pattern short
  enough to appear inside ordinary prose keeps far more than intended.
- Cover the cases that have actually broken: a comment at the first or last line of a file, a
  comment sharing a line with code, a doc comment, and a `~keep` inside a string literal.
