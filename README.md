<div align="center">

<img src="assets/banner.svg" alt="uncomment — strip the noise, keep the code" width="820">

**Strip the noise. Keep the code.**

uncomment removes comments from source code using tree-sitter's AST — so it is 100% accurate and
**never** touches comment-like text inside strings. It keeps what matters by default (TODO/FIXME,
docs, and linting directives) across **300+ languages**, with parallel processing and a safe dry-run
mode.

AST-accurate&nbsp;·&nbsp;306 languages&nbsp;·&nbsp;zero false positives&nbsp;·&nbsp;smart preservation&nbsp;·&nbsp;parallel&nbsp;·&nbsp;dry-run

[![crates.io](https://img.shields.io/crates/v/uncomment?style=flat-square&color=2dd4bf)](https://crates.io/crates/uncomment)
[![npm](https://img.shields.io/npm/v/uncomment-cli?style=flat-square&color=2dd4bf&label=npm)](https://www.npmjs.com/package/uncomment-cli)
[![PyPI](https://img.shields.io/pypi/v/uncomment?style=flat-square&color=2dd4bf)](https://pypi.org/project/uncomment/)
[![CI](https://img.shields.io/github/actions/workflow/status/Goldziher/uncomment/ci.yml?style=flat-square&label=CI)](https://github.com/Goldziher/uncomment/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](./LICENSE)
[![Sponsor](https://img.shields.io/badge/Sponsor-%E2%9D%A4-ff69b4?style=flat-square&logo=github-sponsors)](https://github.com/sponsors/Goldziher)

[Install](#installation)&nbsp;·&nbsp;[Features](#features)&nbsp;·&nbsp;[Usage](#usage)&nbsp;·&nbsp;[Configuration](#configuration)&nbsp;·&nbsp;[How it works](#how-it-works)&nbsp;·&nbsp;[Contributing](#contributing)

</div>

---

## Why uncomment

Regex-based comment strippers guess. They delete a `//` inside a string literal, mangle a URL in a
docstring, or leave a linting directive your CI depends on. uncomment doesn't guess: it parses your
code into a real syntax tree and removes only the nodes that are genuinely comments.

Originally built to clean up AI-generated code drowning in explanatory comments, it now works on
anything with a tree-sitter grammar.

## Features

- **100% accurate** — tree-sitter AST parsing identifies comments structurally, not by pattern matching
- **No false positives** — never removes comment-like content from strings
- **Smart preservation** — keeps TODO/FIXME, docs, and language-specific linting directives by default
- **306 languages** — powered by [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack), grammars downloaded on demand
- **Parallel** — multi-threaded processing that scales across cores
- **Safe** — dry-run mode with line-by-line diffs previews every change before you write
- **Configurable** — hierarchical TOML config with a smart `init` command
- **Built-in benchmarking** — optional performance analysis and profiling tools

## Installation

| Channel | Command |
| ------- | ------- |
| Homebrew (macOS/Linux) | `brew tap goldziher/tap && brew install uncomment` |
| Cargo (Rust) | `cargo install uncomment` |
| npm (Node.js) | `npm install -g uncomment-cli` |
| pip (Python) | `pip install uncomment` |

Run without installing:

```bash
npx -y uncomment-cli@latest .
uvx uncomment .
```

Add `--dry-run` to preview changes before writing.

<details>
<summary><b>Build from source</b></summary>

```bash
git clone https://github.com/Goldziher/uncomment.git
cd uncomment
cargo install --path .
```

Requires Rust 1.70+. npm and pip packages download pre-built binaries automatically.

</details>

## Quick Start

```bash
# Generate a configuration file tuned to your project
uncomment init

# Remove comments from a directory
uncomment src/

# Preview changes as a diff, write nothing
uncomment src/ --dry-run --diff
```

## Usage

```bash
# Single file
uncomment file.py

# Multiple files / globs
uncomment src/*.py

# Also strip doc comments and docstrings
uncomment --remove-doc file.py

# Also remove TODO and FIXME comments (preserved by default)
uncomment --remove-todo --remove-fixme file.py

# Add custom patterns to preserve
uncomment --ignore "HACK" --ignore "WARNING" file.py

# Process an entire tree with all CPU cores
uncomment . -j 0
```

Run `uncomment --help` for the full, grouped list of options.

<details>
<summary><b>Configuring with <code>init</code></b></summary>

The `init` command detects the languages in your project and writes a matching `.uncommentrc.toml`:

```bash
# Smart detection — includes only the languages it finds
uncomment init

# All 49 built-in languages
uncomment init --comprehensive

# Interactive selection
uncomment init --interactive

# Custom output location / overwrite
uncomment init --output config/uncomment.toml --force
```

</details>

<details>
<summary><b>Optional benchmarking tools</b></summary>

Development binaries for benchmarking and profiling are gated behind the `bench-tools` feature so
they are not installed for regular users:

```bash
# Install with extras
cargo install uncomment --features bench-tools

# Or run locally
cargo run --release --features bench-tools --bin benchmark -- --target /path/to/repo --iterations 3
cargo run --release --features bench-tools --bin profile -- /path/to/repo
```

</details>

## Supported Languages

uncomment ships with 49 built-in language configurations and can process any of the **306 languages**
in [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack) — grammars
are downloaded automatically on first use, and any language can be added via configuration.

<details>
<summary><b>49 built-in languages</b></summary>

Python (`.py`, `.pyw`, `.pyi`, `.pyx`, `.pxd`) · JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`) ·
TypeScript (`.ts`, `.tsx`, `.mts`, `.cts`, `.d.ts`) · Rust (`.rs`) · Go (`.go`) · Java (`.java`) ·
C (`.c`, `.h`) · C++ (`.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`) · C# (`.cs`) ·
Ruby (`.rb`, `.rake`, `.gemspec`) · PHP (`.php`, `.phtml`) · Elixir (`.ex`, `.exs`) · TOML (`.toml`) ·
JSON (`.json`) · JSON with Comments (`.jsonc`) · YAML (`.yml`, `.yaml`) ·
HCL/Terraform (`.hcl`, `.tf`, `.tfvars`) · Makefile (`Makefile`, `.mk`) ·
Shell/Bash (`.sh`, `.bash`, `.zsh`) · Haskell (`.hs`, `.lhs`) · HTML (`.html`, `.htm`, `.xhtml`) ·
CSS (`.css`) · XML (`.xml`, `.xsd`, `.xsl`, `.xslt`, `.svg`) · SQL (`.sql`) · Kotlin (`.kt`, `.kts`) ·
Swift (`.swift`) · Lua (`.lua`) · Nix (`.nix`) · PowerShell (`.ps1`, `.psm1`, `.psd1`) ·
Protobuf (`.proto`) · INI-like configs (`.ini`, `.cfg`, `.conf`) · Dockerfile (`Dockerfile`) ·
Scala (`.scala`, `.sc`) · Dart (`.dart`) · R (`.r`, `.R`) · Julia (`.jl`) · Zig (`.zig`) ·
Clojure (`.clj`, `.cljs`, `.cljc`, `.edn`) · Elm (`.elm`) · Erlang (`.erl`, `.hrl`) · Vue (`.vue`) ·
Svelte (`.svelte`) · SCSS (`.scss`) · LaTeX (`.tex`, `.sty`, `.cls`) · Fish (`.fish`) ·
Perl (`.pl`, `.pm`) · Groovy (`.groovy`, `.gradle`) · OCaml (`.ml`, `.mli`) ·
Fortran (`.f90`, `.f95`, `.f03`, `.f08`)

</details>

## Preservation Rules

Certain comments are **never removed by default** — uncomment protects the ones your tooling and
teammates rely on.

**Always preserved:**

- Comments containing `~keep`
- `TODO` (unless `--remove-todo`), `FIXME` (unless `--remove-fixme`)
- Documentation comments (unless `--remove-doc`)

<details>
<summary><b>Linting &amp; formatter directives (always preserved)</b></summary>

| Language | Directives |
| -------- | ---------- |
| Go | `//nolint`, `//golangci-lint`, `//staticcheck`, `//go:generate` |
| Python | `# noqa`, `# type: ignore`, `# mypy:`, `# pyright:`, `# ruff:`, `# pylint:`, `# flake8:`, `# fmt: off/on`, `# black:`, `# isort:`, `# bandit:`, `# pyre-ignore` |
| JS/TS | `eslint-disable*`, `@ts-ignore`, `@ts-expect-error`, `@ts-nocheck`, `/// <reference`, `prettier-ignore`, `biome-ignore`, `deno-lint-ignore`, `v8/c8/istanbul ignore` |
| Rust | `#[allow]`, `#[deny]`, `#[warn]`, `#[forbid]`, `#[cfg]`, `clippy::`, `#[rustfmt::skip]` |
| Java | `@SuppressWarnings`, `@SuppressFBWarnings`, `//noinspection`, `// checkstyle:` |
| C/C++ | `// NOLINT`, `// NOLINTNEXTLINE`, `#pragma`, `// clang-format off/on` |
| Shell | `# shellcheck disable`, `# hadolint ignore` |
| YAML | `# yamllint disable/enable` |
| HCL/Terraform | `# tfsec:ignore`, `# checkov:skip`, `# trivy:ignore`, `# tflint-ignore` |
| Ruby | `# rubocop:disable/enable`, `# reek:`, `# standard:disable/enable` |

</details>

## Configuration

uncomment reads a hierarchical TOML configuration, merged highest-to-lowest precedence:

1. Command-line flags
2. Local `.uncommentrc.toml` (closest to the file being processed wins)
3. Global `~/.config/uncomment/config.toml`
4. Built-in defaults

```toml
[global]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = ["IMPORTANT", "NOTE", "WARNING"]
use_default_ignores = true
respect_gitignore = true

[languages.python]
extensions = ["py", "pyw", "pyi"]
preserve_patterns = ["noqa", "type:", "pragma:", "pylint:"]

[patterns."tests/**/*.py"]
# Keep all comments in test files
remove_todos = false
remove_fixme = false
remove_docs = false
```

<details>
<summary><b>Adding a language via configuration</b></summary>

Any of the 306 tree-sitter-language-pack languages works — grammars download automatically on first
use, no manual grammar setup:

```toml
[languages.hare]
name = "Hare"
extensions = ["ha"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO", "FIXME"]
```

</details>

## How It Works

Unlike regex-based tools, uncomment builds a proper Abstract Syntax Tree of your code with
tree-sitter, so it distinguishes:

- Real comments vs comment-like content in strings
- Documentation comments vs regular comments
- Inline comments vs standalone comments
- Language-specific metadata that must be preserved

The pipeline is modular: a **language registry** (49 built-ins + on-demand grammars) feeds an
**AST visitor** that finds comment nodes, a **preservation engine** decides what to keep, and an
**output generator** emits clean code.

## Git Hooks

<details>
<summary><b>pre-commit</b></summary>

```yaml
repos:
  - repo: https://github.com/Goldziher/uncomment
    rev: v3.3.0
    hooks:
      - id: uncomment
```

</details>

<details>
<summary><b>Lefthook</b></summary>

```yaml
pre-commit:
  commands:
    uncomment:
      run: uncomment {staged_files}
      stage_fixed: true
```

</details>

## Performance

AST parsing costs a little more than regex, but the tool is fast and scales well with threads.

- Small files (<1000 lines): ~20-30ms
- Large files (>10000 lines): ~100-200ms

| Threads | Files/second | Speedup |
| ------- | ------------ | ------- |
| 1 | 1,500 | 1.0× |
| 4 | 3,900 | 2.6× |
| 8 | 5,100 | 3.4× |

_Benchmarked on a large enterprise codebase of ~5,000 mixed-language files._ Measure your own with
the built-in `benchmark` and `profile` tools (see [optional benchmarking tools](#usage)).

## Development

```bash
cargo build              # Debug build
cargo test               # Run the test suite
cargo test -- --ignored  # Include network-dependent tests
cargo clippy             # Lint
cargo fmt --all          # Format
```

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for local development, automation hooks, and release
procedures.

## Contributing

Issues and pull requests are welcome. If uncomment is useful to you, consider
[sponsoring development](https://github.com/sponsors/Goldziher) — it helps keep the project
maintained for the community.

## License

[MIT](./LICENSE)
