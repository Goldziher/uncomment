---
priority: high
aliases: [f]
usage: "/fix"
description: "Auto-fix formatting and linting issues"
---

# Fix

Automatically fix formatting and linting issues.

## Steps

1. Run `cargo fmt --all` to auto-format all Rust code
2. Run `cargo clippy --fix --allow-dirty --allow-staged` to auto-fix clippy warnings where possible
3. Report what was changed:
   - List files that were reformatted
   - List clippy fixes that were applied
   - List any remaining warnings that could not be auto-fixed
