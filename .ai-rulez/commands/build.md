---
type: Reference
title: Build
description: Build the project and report any errors
x-ai-rulez:
  kind: command
  id: build
  metadata:
    priority: high
    aliases:
      - b
    usage: /build
---

## Build

Build the project and report any compilation errors.

### Steps

1. Run `cargo build --release` to build the project in release mode
2. If the build fails:
   - Show the full compiler error output
   - Analyze the errors and suggest fixes
3. If the build succeeds, report the binary location and size
