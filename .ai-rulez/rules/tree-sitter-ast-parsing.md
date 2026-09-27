---
priority: critical
---

# Tree-Sitter AST Parsing

Comments are found by parsing, never by matching text. That is the tool's entire reason to exist: a
`TODO` inside a string literal, a `#` inside a shell here-doc and a `//` inside a URL are not
comments, and no amount of regex tightening makes them distinguishable.

## How it is wired

- `Processor` owns one reused `tree_sitter::Parser` and re-points it per language; there is no
  parsed-tree cache, and a file is parsed exactly once per run.
- Traversal is `CommentVisitor` in `src/ast/visitor.rs`. Node kinds come from the language's
  `comment_nodes` / `doc_comment_nodes` — declared in the constructors in
  `src/languages/config.rs`, or by the user under `[languages.<name>]`.
- Everything downstream — removal, `scan`, `keep`, `lint` — consumes `Processor::inspect`, so they
  cannot disagree about what a comment is. Add a consumer there, not a second traversal.

## Rules

- Never locate a comment by text search. Regex over a comment's *text*, once the AST has handed it
  to you, is fine and is how `lint` reads a tag — the distinction is finding versus inspecting.
- Never edit source without the parse. Byte ranges come from the tree; apply them through
  `crate::edit`, which rejects overlapping, out-of-bounds and non-character-boundary ranges rather
  than corrupting a file.
- Offsets are **bytes**, not characters. Slicing a multi-byte file at a character index panics.
- Node kinds differ per grammar and are not guessable — `comment`, `line_comment`, `block_comment`,
  and Python's docstring is a `string`. Inspect the real tree before declaring them.
