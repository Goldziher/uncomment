---
priority: medium
---

# Configuration System

uncomment uses a hierarchical TOML-based configuration system with layered precedence.

## Precedence (highest to lowest)

1. **CLI flags** — Command-line arguments override everything
2. **Local config** — `.uncommentrc.toml` in the project directory
3. **Global config** — `~/.config/uncomment/config.toml`
4. **Defaults** — Built-in default values

## Implementation

- Configuration is managed in `src/config.rs` via the `Config` struct
- Uses `serde` for TOML deserialization
- Template methods (`template()`, `smart_template()`, `comprehensive_template()`) generate example configs

## Guidelines

- When adding new config options, update the `Config` struct in `src/config.rs`
- Add the option to all template methods so users can discover it
- Document new options in the `examples/` directory
- Always provide sensible defaults — the tool should work with zero configuration
- Use `serde(default)` for optional fields to ensure backward compatibility
