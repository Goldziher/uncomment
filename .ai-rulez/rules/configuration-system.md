---
priority: medium
---

# Configuration System

Layered TOML, defined in `src/config/` and resolved per file by `ConfigManager`
(`src/config/manager.rs`).

## Precedence (highest to lowest)

1. **CLI flags.**
2. **Project config**, nearest to the file being processed first, then each ancestor outward. A
   nested config overrides only the keys it actually contains. `CONFIG_FILE_NAMES` is the
   authority on the file names: `.uncomment.toml` is the preferred one, and `.uncommentrc.toml`
   and `uncomment.toml` are read as deprecated fallbacks. A directory holding more than one uses
   the highest-precedence name outright — the others are not merged in. Write the preferred name
   in anything new, including fixtures.
3. **Global config** at the platform config directory (`dirs::config_dir()`) —
   `~/.config/uncomment/config.toml` on Linux,
   `~/Library/Application Support/uncomment/config.toml` on macOS.
4. **Built-in defaults.**

Two tables do not follow this.

`[languages.*]` is read before any file is, because a declared extension has to be known while
files are still being collected — `ConfigManager::discover_language_sources` gathers it from the
invocation directory, its ancestors, the global config, and every config under the paths being
processed. A config reached only later, during per-file resolution, is too late and its
`[languages]` section is reported as ignored.

`[lint]` amends the table above it key by key, each severity in `[lint.rules]` included, rather
than replacing it — so a subdirectory can switch one rule off and keep inheriting `enabled` and
`key_pattern`. `[patterns."<glob>"]` is the opposite: last match wins.

## Adding an option

- Wire form first: every `[global]` key is `Option<T>` on `GlobalConfigFile`, and its presence is
  recorded in `SpecifiedFlags`. Layering needs "key absent" to differ from "key set to the default
  value" — `#[serde(default)]` on a `bool` collapses the two and silently erases the outer
  config's setting.
- Every config struct carries `#[serde(deny_unknown_fields)]`, so a typo is an error rather than a
  no-op. Keep it that way, and reject invalid values in `Config::validate` rather than at use.
- Add the key to the `template*`/`comprehensive_template*` generators, or `uncomment init` will
  not mention it.
