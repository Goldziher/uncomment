---
priority: high
aliases: [t]
usage: "/test"
description: "Run the test suite and report results"
---

# Test

Run the full test suite and report results.

## Steps

1. Run `cargo test` to execute all unit and integration tests
2. If any tests fail:
   - Show the failing test name and error output
   - Analyze the failure and suggest a fix
3. If all tests pass, report the total number of tests run and time taken
4. If there are warnings, list them
