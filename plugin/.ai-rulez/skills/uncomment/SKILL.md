---
type: Playbook
title: Uncomment Source Comment Cleanup
description: Inventory, preserve, lint and remove source comments using Uncomment's tree-sitter CLI or Rust library. Use for comment cleanup and preservation policies without changing comment-like text in strings.
x-ai-rulez:
  kind: skill
  id: uncomment
  metadata:
    name: uncomment
    license: MIT
    metadata:
      author: Goldziher
      repository: https://github.com/Goldziher/uncomment
---

## Uncomment Source Comment Cleanup

Uncomment uses tree-sitter to find source comments. This skill targets Uncomment
3.10 and newer, including `scan`, `keep`, `lint` and the `--check` gate. Install the
CLI separately; this plugin provides instructions and has no MCP server.

```bash
cargo install uncomment --version '^3.10'
# Binary wrappers are available as uncomment-cli on npm and uncomment on PyPI.
uncomment --version
```

### Inspect and preview

Start with the paths the user selected. An ordinary removal invocation writes
files; `scan`, `--dry-run`, and `--check` let you inspect the planned cleanup.

```bash
uncomment scan src/ --only removable --format jsonl --output comments.jsonl
uncomment src/ --dry-run --diff
```

Scan records contain `id`, `path`, `line`, `end_line`, `start_byte`, `end_byte`,
`kind`, `verdict` and `text`. Preserved records also include `reason`. Positions are
one-based lines and zero-based byte offsets. Use IDs to select comments; IDs survive
edits above a comment but depend on its path, exact text and occurrence index.

### Preserve selected comments

Use a plain comment marker for comments that must survive cleanup:

```rust
// This workaround must stay until the upstream fix ships. ~keep
// ~keep
/// Documentation that must survive --remove-doc.
```

A standalone marker uses the language's plain comment token, such as `//` or `#`,
not a documentation prefix such as `///`. Quoted mentions of `~keep` are prose,
not active markers. Let `keep` handle block comments and docstrings so it can
place and verify the marker without editing their runtime contents.

```bash
uncomment keep src/ --id 'ID_FROM_SCAN'
uncomment keep src/ --match 'workaround' --dry-run --diff
uncomment keep src/ --from selected-comments.jsonl
```

`--from` marks every ID in the supplied JSON or JSONL records; select the records
first. Extra decision fields do not turn an ID into a rejection. Missing IDs fail
unless `--skip-missing` is explicitly requested.

### Apply cleanup or gate it

```bash
uncomment src/ --dry-run --diff
uncomment src/
uncomment --check src/ --format json
uncomment --check --changed-only --base origin/main .
```

Defaults preserve TODO/FIXME, documentation, common lint directives and active
`~keep` markers. `--remove-todo`, `--remove-fixme` and `--remove-doc` opt into removing
those categories; `--ignore PATTERN` adds case-sensitive substring preservation.
`--no-default-ignores` disables the built-in preservation patterns.

The removal gate exits 0 when nothing would be removed, 1 when removable comments
exist, and 2 when inspection could not complete. Check reported file counts;
zero matched files are not evidence that the intended tree passed. In 3.10.3, a
missing input path can print an error and still exit 0, so also verify paths exist
and the report covers the expected files. Current main source builds reject a
missing explicit path with exit 2; that fix has not been released yet.

### Lint tag conventions

Tag linting removes nothing and is inactive until `[lint] enabled = true` is
configured. Use [configuration](references/configuration.md) for layering,
exclusions and rule settings.

```bash
uncomment lint src/ --format json
uncomment lint src/ --fix --todo-key 'PROJECT-123'
```

Use `--fix` only when rewriting tag comments is part of the requested work.
The key must be a real issue key chosen for that project.

For read-only Rust integration, see [library usage](references/library.md).
