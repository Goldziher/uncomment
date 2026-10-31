<!--
AI-RULEZ :: GENERATED FILE — DO NOT EDIT
Content-Hash: blake3:c2b7c3bed8e321e5c3e141562df4cc9908f270936bd0dd786bbbdbb9f263394c
Source-Hash: blake3:d94e47c9b5760f2cf724e055e236d9021312752c3c3f065f46d95d0ea5818506
Schema-Version: v1
-->

# Rust Library Integration

```toml
[dependencies]
uncomment = "3.10"
anyhow = "1"
```

`Processor` and `ConfigManager` expose the same AST-based processing used by the
CLI. `process_file_with_config` reads the source and returns a `ProcessedFile`; it
does not write the processed bytes back.

```rust
use std::path::Path;
use uncomment::{Processor, config::ConfigManager};

fn preview() -> anyhow::Result<()> {
    let manager = ConfigManager::new(Path::new("."))?;
    let mut processor = Processor::new_with_config(&manager);
    let result = processor.process_file_with_config(Path::new("src/lib.rs"), &manager, None)?;
    println!("{}", result.processed_content);
    Ok(())
}
```

Use `Processor::inspect` for individual comment records and preservation reasons,
or `Processor::plan_removals` for byte-range edits. Both accept source text, a path
and a `ResolvedConfig`. Preserve byte offsets; they are not character indices.

The CLI's `scan`, `keep`, `lint` and removal planner share this inspection surface,
so use it rather than implementing another comment detector with text matching.
The language pack is compiled into the binary; no runtime grammar download or
custom MCP server is part of this library API.
