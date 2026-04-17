---
priority: high
aliases: [l]
usage: "/lint"
description: "Run all linters and formatters"
---

# Lint

Run all linters and formatters to check code quality.

## Steps

1. Run `cargo fmt --all -- --check` to verify formatting
2. Run `cargo clippy -- -D warnings` to check for lint issues
3. If there are formatting issues, report them and suggest running `cargo fmt --all`
4. If there are clippy warnings, list each one with the file, line, and suggested fix
5. Report overall status: pass or fail
