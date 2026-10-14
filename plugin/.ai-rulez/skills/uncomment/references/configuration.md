# Configuration and CLI Selection

The preferred project file is `.uncomment.toml`. Generate it with
`uncomment init`; `--comprehensive` includes every option. Existing files are
protected unless `--force` is supplied.

```toml
[global]
remove_todos = false
remove_fixme = false
remove_docs = false
use_default_ignores = true
preserve_patterns = ["WORKAROUND", "Copyright", "License"]
exclude = ["generated/**", "vendor/**"]

[lint]
enabled = true
```

Global options layer from built-in defaults to the platform global config, project
ancestor configs, the nearest project config, and CLI flags. `-c, --config FILE`
selects a single config instead of automatic discovery. `.uncommentrc.toml` and
`uncomment.toml` are deprecated fallbacks; new projects should use `.uncomment.toml`.
A directory containing multiple recognized names uses the highest-precedence one,
not a merge of those files.

The global config lives under the platform config directory: typically
`~/.config/uncomment/config.toml` on Linux and
`~/Library/Application Support/uncomment/config.toml` on macOS.

`preserve_patterns` and `--ignore` match case-sensitive substrings of an already
identified comment, not regular expressions over source code. Short patterns can
preserve more comments than intended. Use narrow patterns for a named convention.

## File selection

- Paths can name files, directories or globs. Quote globs when Uncomment should expand them.
- `.gitignore` is respected unless `--no-gitignore` is supplied.
- `--exclude GLOB` adds exclusions to `[global] exclude`.
- Nested Git repositories are skipped unless `--traverse-git-repos` is supplied.
- `-j, --threads N` selects workers; 0 uses all cores, and the default is 1.
- No `--language` or stdin mode is declared in the CLI; use source file extensions
  or configured language registrations.

For branch gates, use `--changed-only --base REF` to restrict files, or
`--changed-lines --base REF` to restrict findings to changed lines. `--staged`
selects the index diff rather than the worktree diff.

## Tag linting

`[lint] enabled = true` activates the default tag rules. `uncomment lint` then
checks canonical tags and issue keys, including references to the current branch's
own issue key. Without this setting, matching files have no active tag policy.

```toml
[lint]
enabled = true

[lint.rules]
todo_missing_key = "error"
todo_self_reference = "warn"
```

`--baseline FILE` makes already-recorded violations informational;
`--write-baseline --baseline FILE` records the current violations and exits 0.
Use baselines only when the project intentionally accepts that existing debt.
