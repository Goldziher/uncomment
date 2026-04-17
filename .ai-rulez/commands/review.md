---
priority: high
aliases: [rev]
usage: "/review"
description: "Review current changes for correctness, style, and potential issues"
---

# Review

Review all staged and unstaged changes in the current repository.

## Steps

1. Run `git diff` and `git diff --staged` to see all changes
2. For each changed file, review:
   - Correctness: Does the code do what it intends? Are there logic errors?
   - Style: Does it follow Rust conventions (snake_case, idiomatic patterns, `?` for error propagation)?
   - Safety: No `unwrap()`/`expect()` in production paths, proper error handling with `anyhow`/`thiserror`
   - Tests: Are new features or bug fixes covered by tests?
   - Documentation: Are public APIs documented with `///` comments?
3. Check for common issues:
   - Unused imports or dead code
   - Missing error context (`.context()` / `.with_context()`)
   - Hardcoded values that should be configurable
   - Version synchronization if any manifest was modified
4. Provide a summary with actionable feedback, categorized by severity (critical, warning, suggestion)
