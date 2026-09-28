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
[![License: MIT](https://img.shields.io/badge/License-MIT-2dd4bf?style=flat-square)](./LICENSE)
[![Sponsor](https://img.shields.io/badge/Sponsor-%E2%9D%A4-2dd4bf?style=flat-square&logo=github-sponsors)](https://github.com/sponsors/Goldziher)

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
- **306 languages** — powered by [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack), every grammar compiled into the binary
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

Prefer prebuilt binaries? [`cargo binstall uncomment`](https://github.com/cargo-bins/cargo-binstall) downloads a
release archive instead of compiling from source.

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

# Skip paths entirely, whatever the comments in them
uncomment . --exclude "playground/**" --exclude "thirdparty/**"

# Process an entire tree with all CPU cores
uncomment . -j 0

# Fail (exit 1) if any comment would be removed, writing nothing
uncomment --check src/
```

Run `uncomment --help` for the full, grouped list of options.

<details>
<summary><b>Configuring with <code>init</code></b></summary>

The `init` command detects the languages in your project and writes a matching `.uncomment.toml`:

```bash
# Smart detection — includes only the languages it finds
uncomment init

# Every global and pattern option, fully commented
uncomment init --comprehensive

# Interactive selection
uncomment init --interactive

# Custom output location / overwrite
uncomment init --output config/uncomment-rules.toml --force
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

uncomment ships with 55 built-in language configurations and can process any of the **306 languages**
in [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack) — every
grammar is compiled into the binary, so nothing is downloaded, built or cached at runtime, and any
language can be added via configuration.

<details>
<summary><b>55 built-in languages</b></summary>

Python (`.py`, `.pyw`, `.pyi`, `.pyx`, `.pxd`) · JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`) ·
TypeScript (`.ts`, `.mts`, `.cts`, `.d.ts`, `.d.mts`, `.d.cts`) · TSX (`.tsx`) · Rust (`.rs`) ·
Go (`.go`) · Go templates (`.tpl`, `.tmpl`, `.gotmpl`, `.gohtml`) · Java (`.java`) ·
C (`.c`, `.h`) · C++ (`.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`) · C# (`.cs`) ·
Ruby (`.rb`, `.rake`, `.gemspec`) · PHP (`.php`, `.phtml`) · Elixir (`.ex`, `.exs`) · TOML (`.toml`) ·
JSON (`.json`) · JSON with Comments (`.jsonc`) · YAML (`.yml`, `.yaml`) ·
HCL/Terraform (`.hcl`, `.tf`, `.tfvars`) · Makefile (`Makefile`, `makefile`, `GNUmakefile`, `.mk`) ·
Shell/Bash (`.sh`, `.bash`, `.zsh`, `.bashrc`, `.zshrc`, `.zshenv`) · Haskell (`.hs`, `.lhs`) · HTML (`.html`, `.htm`, `.xhtml`) ·
CSS (`.css`) · XML (`.xml`, `.xsd`, `.xsl`, `.xslt`, `.svg`) · SQL (`.sql`) · Kotlin (`.kt`, `.kts`) ·
Objective-C (`.m`) · Swift (`.swift`) · Lua (`.lua`) · Nix (`.nix`) · PowerShell (`.ps1`, `.psm1`, `.psd1`) ·
Protobuf (`.proto`) · INI-like configs (`.ini`, `.cfg`, `.conf`) · Dockerfile (`Dockerfile`, `Dockerfile.*`) ·
Starlark/Bazel (`BUILD`, `BUILD.bazel`, `WORKSPACE`, `WORKSPACE.bazel`, `WORKSPACE.bzlmod`, `MODULE.bazel`, `.bzl`, `.bazel`, `.star`) ·
Java properties (`.properties`) ·
Markdown (`.md`, `.markdown`, `.mdown`, `.mkd`) ·
Scala (`.scala`, `.sc`) · Dart (`.dart`) · R (`.r`, `.R`) · Julia (`.jl`) · Zig (`.zig`) ·
Clojure (`.clj`, `.cljs`, `.cljc`, `.edn`) · Elm (`.elm`) · Erlang (`.erl`, `.hrl`) · Vue (`.vue`) ·
Svelte (`.svelte`) · SCSS (`.scss`) · LaTeX (`.tex`, `.sty`, `.cls`) · Fish (`.fish`) ·
Perl (`.pl`, `.pm`) · Groovy (`.groovy`, `.gradle`) · OCaml (`.ml`, `.mli`) ·
Fortran (`.f90`, `.f95`, `.f03`, `.f08`)

</details>

Objective-C uses the `objc` grammar. The `.m` extension is also used by MATLAB;
Uncomment treats it as Objective-C by default. When processing a mixed project,
pass only the Objective-C paths. Headers (`.h`) retain the C configuration;
Objective-C++ (`.mm`) is not included in built-in support.

Markdown has no comment of its own — `<!-- … -->` is raw HTML to CommonMark, and the grammar reports
it as the same `html_block` that carries a `<div align="center">` badge row. A block is treated as a
comment only when it is *nothing but* one comment, so embedded HTML, a `<!DOCTYPE>`, an unterminated
`<!--` (which runs to the end of the document) and `<!-- x --><div>kept</div>` are all left alone. A
comment inside a paragraph or a table cell is inline content rather than an `html_block`, so it is
out of reach; `.mdx` is not supported at all, because no grammar in the pack parses `{/* … */}`.

In a Go template a comment is only a comment inside an action, so a bare `/* … */` is literal output
text and `{{/* … */}}` is the only form. The grammar models the `{{` and `}}` as *siblings* of the
comment rather than as part of it, so the whole action is removed — deleting only the comment would
leave `{{}}`, which Helm refuses to render. One spacing caveat: `{{ /* spaced */ }}` and `{{-/* x */-}}`
produce no comment node at all and are left alone. A comment with code on *both* sides keeps the
spaces around it (`hello {{/* x */}} world` → `hello  world`), because in a template that whitespace is
rendered output. One that ends the line does not: `hello {{/* x */}}` leaves `hello`, since a trailing
comment takes its separator with it like any other language's.

## Preservation Rules

Certain comments are **never removed by default** — uncomment protects the ones your tooling and
teammates rely on.

**Always preserved:**

- Comments containing `~keep`
- `TODO` (unless `--remove-todo`), `FIXME` (unless `--remove-fixme`)
- Documentation comments (unless `--remove-doc`)

### The `~keep` marker

Put `~keep` on a `//`-style comment, not on a `///` or `/** */` doc comment. `~keep` is
plain comment text, so a marker written inside a doc comment is republished by every tool
that consumes doc comments — rustdoc, OpenAPI schemas, generated API clients, editor
hover text.

A marker on its own line protects the comment directly beneath it, which keeps it out of
anything rendered:

```rust
// ~keep
/// Parent element ID for hierarchical relationships.
pub parent_id: Option<String>,
```

The marker also extends across a contiguous run of comments, so one `~keep` protects a
whole multi-line block. A blank line or any code between comments ends the run.

Inside a doc comment the marker is usually redundant, since doc comments are preserved
anyway unless `--remove-doc` is set. uncomment strips such a marker from the doc text and
reports it (`stripped 2 redundant ~keep markers`), leaving the comment itself untouched.
Two cases are left alone: a marker is kept when `--remove-doc` is set, because there it is
the only thing protecting that doc comment, and prose *about* the marker is never rewritten
— a line containing backticks, or a commented-out code sample, is read as documentation
rather than as a directive.

<details>
<summary><b>Linting &amp; formatter directives (always preserved)</b></summary>

| Language | Directives |
| -------- | ---------- |
| Go | `//nolint`, `//golangci-lint`, `//staticcheck`, `//go:generate` |
| Python | `# noqa`, `# type: ignore`, `# mypy:`, `# pyright:`, `# ruff:`, `# pylint:`, `# flake8:`, `# fmt: off/on`, `# black:`, `# isort:`, `# bandit:`, `# nosec`, `# pyre-ignore` |
| JS/TS | `eslint-disable*`, `@ts-ignore`, `@ts-expect-error`, `@ts-nocheck`, `/// <reference`, `prettier-ignore`, `biome-ignore`, `deno-lint-ignore`, `v8/c8/istanbul ignore` |
| Rust | `#[allow]`, `#[deny]`, `#[warn]`, `#[forbid]`, `#[cfg]`, `clippy::`, `#[rustfmt::skip]` |
| Java | `@SuppressWarnings`, `@SuppressFBWarnings`, `//noinspection`, `// checkstyle:` |
| C/C++ | `// NOLINT`, `// NOLINTNEXTLINE`, `#pragma`, `// clang-format off/on` |
| Shell | `# shellcheck disable`, `# hadolint ignore` |
| YAML | `# yamllint disable/enable` |
| HCL/Terraform | `# tfsec:ignore`, `# checkov:skip`, `# trivy:ignore`, `# tflint-ignore` |
| Ruby | `# rubocop:disable/enable`, `# reek:`, `# standard:disable/enable` |
| Markdown | `<!-- markdownlint-* -->`, `<!-- prettier-ignore -->`, `<!-- vale off/on -->`, `<!-- START/END doctoc -->` |

</details>

## Subcommands

### `uncomment scan` — comment inventory

Reports every comment in the tree with the verdict a real run would reach, as JSONL, JSON or text. Each comment gets a stable id that survives edits elsewhere in the file.

```bash
# Inventory everything
uncomment scan src/ --format jsonl -o scan.jsonl

# Only the comments a run would remove
uncomment scan src/ --only removable

# Collapse identical comments into groups, sorted by frequency
uncomment scan src/ --group-identical --only removable
```

**Flags:**

- `--format jsonl|json|text` — output format (default: jsonl)
- `--only removable|preserved|all` — filter by verdict (default: all)
- `--group-identical` — collapse equivalent comments into one record per distinct text, with a site count and list
- `-o FILE` — write report to FILE instead of stdout

**ID scheme:** IDs are derived from the file path, comment bytes, and occurrence index within the file — no line numbers, so an id survives edits above the comment. Collisions within a file are widened to the full 32-hex identifier; cross-file collisions widen the later claimant.

### `uncomment keep` — write `~keep` markers

Applies `~keep` markers to comments selected by id, substring, or verdict. A line comment gets `~keep` appended; a block, doc or docstring comment gets a marker line directly above it, written with the language's plain line-comment token — or with its block pair (`/* ~keep */`, `<!-- ~keep -->`) when the language has no line form. A block, doc or docstring comment sharing its line with code cannot be marked at all: a marker line above it would attach to the line of code, so the comment is reported as unmarkable instead.

```bash
# Mark everything a scan reported as removable
uncomment keep src/ --from scan.jsonl

# Mark specific comments by id
uncomment keep src/ --id a3f9c1d2ab --id b7e4f8a1cd

# Mark every comment whose text contains a substring
uncomment keep src/ --match "legacy shim"

# Mark everything that would be removed
uncomment keep src/ --all-removable
```

**Flags:**

- `--from FILE` — read comment ids from a scan output file (JSONL or JSON; every field but `id` is ignored)
- `--id ID` — mark the comment with this id (repeatable)
- `--match SUBSTRING` — mark every comment whose text contains SUBSTRING
- `--all-removable` — mark every comment a default run would remove
- `--skip-missing` — warn about ids that no longer resolve instead of failing

**Why a marker line above for docstrings:** A Python docstring is the `string` node that becomes `__doc__` at runtime, so editing its bytes changes what the program reports about itself. A marker line written above the docstring is a plain comment that uncomment reads but the runtime ignores.

### `uncomment --check` — fail when a comment would be removed

Runs exactly as `--dry-run` does and writes nothing, but reports every comment the run would remove as a violation and exits non-zero, so the policy "every comment is either kept on purpose or gone" can gate a commit or a pull request. A comment passes when uncomment's own rules keep it: a `~keep` marker, a linting directive, a documentation comment, a configured preserve pattern, or TODO/FIXME while those are preserved.

```bash
# Everything under src/
uncomment --check src/

# Only files a branch changed, or only the lines it changed
uncomment --check --changed-only --base origin/main .
uncomment --check --changed-lines --base origin/main .

# Only what the next commit changes, line by line (for a pre-commit hook)
uncomment --check --staged --changed-lines .
```

Each violation is one line, sorted by path, then line:

```text
src/main.rs:2:5: // strip me
src/main.rs:3:16: // trailing note
✗ 2 removable comment(s) in 1 file(s) (1 file(s) checked)
```

**Exit codes:** `0` nothing would be removed, `1` something would, `2` the check could not be completed — bad arguments, a rejected config, a file that could not be read, or a failed `git diff` — so a broken gate never reads as a pass.

**Flags:**

- `--check` — report instead of rewrite; rejects `--diff`, and makes `--dry-run` redundant
- `--format text|json` — output format; JSON has the same `violations` / `notes` / `summary` shape as `uncomment lint`
- `--changed-only` — check only files changed against `--base` (default: `origin/HEAD`, else `main`)
- `--changed-lines` — report only comments on lines the diff added or rewrote (implies `--changed-only`); this is what makes the gate adoptable on a codebase that already has comments, because editing a legacy file does not make its old comments your problem
- `--staged` — diff the index against `HEAD` instead of `--base`
- `--base REF` — base ref for `--changed-only` and `--changed-lines`

Excluded paths and files of unsupported types are skipped without a word, so a hook can pass every staged file. `--quiet` prints only the summary. See [Git Hooks](#git-hooks) for a lefthook and a GitHub Actions setup.

### `uncomment lint` — tag comment linting

Checks tag comments (TODO, FIXME, HACK, XXX) against the convention configured under `[lint]`: that the tag is canonical, that it carries an issue key, and that the key is not the issue the current branch is working on. Removes nothing, exits 1 on violations, and works as a pre-commit hook.

```bash
# Check comments, report violations, write nothing
uncomment lint src/

# Fix what can be fixed (canonical tags, injecting --todo-key)
uncomment lint src/ --fix --todo-key PROJ-1234

# Check only what the branch touched
uncomment lint src/ --changed-only

# Record current violations; future runs treat them as informational
uncomment lint src/ --baseline .lint-baseline.json --write-baseline
uncomment lint src/ --baseline .lint-baseline.json
```

**Flags:**

- `--fix` — rewrite what can be rewritten (canonical tag form, injecting `--todo-key` where missing)
- `--todo-key KEY` — issue key to insert into tag comments that have none (with `--fix`)
- `--changed-only` — lint only files changed against `--base`'s merge-base with `HEAD` (default:
  `origin/HEAD`, else `main`), comparing against the working tree — staged, unstaged and untracked
  (but not ignored) edits included, not just what has been committed
- `--changed-lines` — report only violations on lines changed against `--base` (implies
  `--changed-only`, and the same working-tree comparison — an untracked file counts every line)
- `--staged` — diff the index against `HEAD` instead of `--base`
- `--base REF` — base ref for `--changed-only` and `--changed-lines`
- `--baseline FILE` — treat violations recorded here as informational
- `--write-baseline` — record every current violation in the baseline file and exit 0
- `--format text|json` — output format (default: text)

**Five rules:**

1. `tag-not-canonical` — a tag not written the way `canonical_tag` is written. That covers two defects and the message says which: a different spelling, reported as *`FIXME` should be written as `TODO`*, and the canonical spelling in the wrong casing, reported as *`todo` is `TODO` written with the wrong casing*. `--fix` rewrites both.
2. `todo-missing-key` — no issue key at all: not in a group, not spelled out nearby
3. `todo-key-not-upper-case` — a key was found but is not upper case, reported as *issue key `amvp-12` must be upper case*. `--fix` recases it in place.
4. `tag-form-not-canonical` — a key was found near the tag, but the tag is not written as `TAG(KEY):` around it: the key sits bare in the text (`TODO AMVP-12: x`, `TODO: AMVP-12 x`), in brackets (`TODO [AMVP-12] x`), or the tag itself is wrapped in its own `(...)`. `--fix` rewrites the whole form — the wrap, any bracket, and a key already in the leading text — into the single `TAG(KEY):` prefix, reusing the key that was already there rather than inserting a second one from `--todo-key`. A leftover from an earlier, careless rewrite, such as `(TODO(AMVP-1):):`, is reported and cleaned up the same way.
5. `todo-self-reference` — the key must not be the issue the current branch is for (that issue closes when the branch merges, leaving the TODO pointing at a dead ticket)

**Casing:** tags are matched regardless of casing, so `fixme`, `Todo` and `xXx` are tags and each is a `tag-not-canonical` violation whose `--fix` normalises it to `canonical_tag`. A key is recognised the same way: `TODO(amvp-1)` and `todo(amvp-1)` both count as keyed, but a lower-case key is still wrong — it is `todo-key-not-upper-case`, not `todo-missing-key`, and `--fix` recases it without touching anything else. `key_pattern` governs this canonical `TAG(KEY):` group and can be replaced with a custom pattern; the bare-key forms `tag-form-not-canonical` also recognises (`TODO AMVP-12: x`, `TODO: AMVP-12 x`, `TODO [AMVP-12] x`) always use the default `AMVP-12`-shaped key, regardless of a custom `key_pattern`. Set `case_sensitive_tags = true` to go back to matching only the literal casing in `tags`, where `todo:` is not a tag at all.

**Tag position:** a tag word counts only where a tag is written, not where a sentence mentions one. It is a tag when it opens its comment — everything before it on the line is delimiter or decoration (`//`, `#`, `/*`, `{/*`, a `*` continuation, a `-` bullet) — or when it opens a trailing segment of the comment, as in `# noqa: T201  # TODO: fix`. In that head position any casing counts, so `# todo: fix` and `* Hack: works around the driver` are tags. Further into a comment, only the exact spelling in `tags` can be a tag, and only when it reads as one: followed by `:`, `(` or a spaced dash (`# Socks handling - TODO: add a failure type`), or opening a clause after a label or punctuation (`# Note: TODO later`, `// biome-ignore lint/x: TODO remove`, `# fine for now, TODO revisit`). Everything else is prose and is neither reported nor touched by `--fix`: `# Line contains TODO`, `# Missing author in TODO`, `?tenant_id=XXX`, `{"uuid": XXX}`, and any tag word in backticks or quotes (`` `TODO` ``, `'hack'`). The cost is a tag written as a bare trailing word — `# remove later TODO` — which reads exactly like `# Line contains TODO` and is not reported.

**Doc comments:** docstrings and doc comments (`///`, `//!`, `/** */`, Python docstrings) are not inspected by default. They describe an interface, and a tag word in one — "Returns the TODO list", `{"uuid": XXX} will be returned` — is far more often part of that description than tracked work. A plain comment that a language handler files as documentation only by position, such as Go's `// TODO` directly above a `func`, is still linted, and so is a `## TODO:` comment, which in Python, YAML and shell is emphasis rather than Doxygen. Set `include_doc_comments = true` to lint documentation too; the tag-position rule still applies inside it.

A sigil is a separate question from casing and always has been: `\b` sits between `@` and the tag, so `# @TODO: x` is a tag today and `# @todo: x` becomes one now. `--fix` rewrites the tag token alone and leaves the sigil, giving `# @TODO: x`. If `@todo` is prose you do not want linted, `case_sensitive_tags = true` is not the lever — drop `TODO` from `tags` or baseline the occurrences.

**Opt-in:** Linting is off by default. Enable it per file tree with `[lint]` in `.uncomment.toml`:

```toml
[lint]
enabled = true
key_pattern = '^\s*(?:TODO|FIXME|HACK|XXX)\((?<key>[A-Z][A-Z0-9]+-\d+)\)\s*:'
# Optional, default false. With `true`, only the literal casing in `tags` is a tag,
# so `todo:` and `Fixme:` are not flagged at all.
case_sensitive_tags = true
# Optional, default false. With `true`, docstrings and doc comments are linted too.
include_doc_comments = true
```

Note what `enabled = true` alone commits you to: with the default `tags` and `canonical_tag`, every existing `FIXME`, `HACK` and `XXX` in the tree becomes two violations — one for the tag, one for the missing key — and `--fix` rewrites the tag to `TODO`. Every miscased tag counts too, `todo:` and `Fixme:` included. On an existing codebase, reach for `--changed-only` or `--write-baseline` first.

### Large-repo workflow

On a large codebase, deciding which comments to keep is a batch process: scan once, filter the report, then mark what the filter selected.

Two scans, because the two reports answer different questions. A grouped report is for *deciding* — one judgement per distinct wording instead of hundreds. `keep` cannot act on it: a grouped record's `id` is a group id derived from the normalized text, with no path and no occurrence index, so it resolves to no single comment and the run fails with every id unresolved (`--skip-missing` downgrades that to a warning). Feed `keep --from` the ungrouped report.

```bash
# 1. Read the decision surface: one record per distinct comment, by frequency
uncomment scan . --group-identical --only removable

# 2. Inventory every site, which is what keep consumes
uncomment scan . --only removable --format jsonl -o scan.jsonl

# 3. Filter scan.jsonl — delete the lines for comments that should be removed,
#    keep the lines for comments that should be kept. Any JSON-aware tool works;
#    the only field `keep` reads is `id`.

# 4. Apply markers to the comments the filtered report selected
uncomment keep . --from scan.jsonl

# 5. Run the real removal
uncomment .

# 6. Verify: a second scan reports 0 removable comments
uncomment scan . --only removable
```

## Configuration

uncomment reads hierarchical TOML configuration. Precedence, lowest to highest:

1. Built-in defaults
2. Global config at `uncomment/config.toml` under the platform config directory (`dirs::config_dir()`) — `$XDG_CONFIG_HOME` or `~/.config` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows
3. Local `.uncomment.toml` files (from repository root toward the file; inner beats outer)
4. Pattern matches (`[patterns."glob"]`) within those configs, last match wins
5. Language-specific settings (`[languages.name]`)
6. Command-line flags

A directory's config file is named `.uncomment.toml`. Two earlier names are still read — `.uncommentrc.toml`, then `uncomment.toml` — so an existing config keeps working, but each is deprecated and a run that loads one says so once on stderr. A directory holding more than one of the three uses the highest-precedence name outright; the others are ignored rather than merged.

`[lint]` layers differently from the rest: a nested table amends the one above it key by key, each severity in `[lint.rules]` included, so a subdirectory can switch one rule off and still inherit `enabled`, `tags` and `key_pattern` from above.

`[languages.*]` does not follow step 3 either. The language registry is built once, before any file is read, from the configs in the invocation directory and its ancestors plus every config found under the paths being processed — deepest declaration wins. (`--config FILE` replaces that discovery entirely.) A config reached only later, during per-file resolution, comes too late: its `[languages]` section is ignored, with a warning naming the directory.

CLI flags are one-directional: `--remove-doc` sets `remove_docs = true` but an unset flag never clobbers a config-file value back to false.

`exclude` does not layer like the rest either — it is a union. A file matching any `exclude` glob in force is not collected by any subcommand, and a nested config can add an exclusion but never withdraw one. Repeating `--exclude GLOB` adds to the configured list rather than replacing it, and naming an excluded path on the command line does not override it: the setting says which paths the project never wants read, not which ones this invocation skips. Globs use the same dialect as `[patterns."<glob>"]` keys — `*` does not cross a `/`, `**` does — and are anchored the same way, relative to the directory holding the config file that declared them (relative to the invocation directory for `--exclude`, and for a `--config FILE` config). `exclude = ["vendor/**"]` also keeps the directory walk out of `vendor` rather than descending it and discarding the results. Like `[languages.*]`, `exclude` has to be known before collection, so it is read from the same set of configs: the invocation directory and its ancestors plus every config found under the paths being processed.

Every table rejects unknown keys, and a project config that does not parse stops the run instead of degrading to built-in defaults — so a typo such as `enable` for `enabled` in `[lint]` fails every subcommand, not just `lint`.

```toml
[global]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = ["IMPORTANT", "NOTE", "WARNING"]
exclude = ["playground/**", "thirdparty/**"]
use_default_ignores = true
respect_gitignore = true

[languages.python]
name = "Python"
extensions = ["py", "pyw", "pyi"]
comment_nodes = ["comment"]
doc_comment_nodes = ["string"]
preserve_patterns = ["noqa", "type:", "pragma:", "pylint:"]

[patterns."tests/**/*.py"]
# Keep all comments in test files
remove_todos = false
remove_fixme = false
remove_docs = false
```

<details>
<summary><b>Adding a language via configuration</b></summary>

Any of the 306 tree-sitter-language-pack languages works. The grammar is already in the binary, so
there is no grammar to fetch or build. The grammar is looked up by the `name` field lowercased — not
by the section key — so `name` has to be the pack's own name for the language, or the name of a
built-in you are overriding. A `name` that matches neither is reported on stderr rather than
silently ignored:

```toml
[languages.hare]
name = "Hare"
extensions = ["ha"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO", "FIXME"]
```

A language can claim whole filenames instead of, or as well as, extensions — the only way to reach a
file that has no extension at all:

```toml
[languages.starlark]
name = "Starlark"
extensions = ["bzl", "bazel", "star"]
filenames = ["BUILD", "BUILD.bazel", "WORKSPACE", "MODULE.bazel", "Tiltfile"]
comment_nodes = ["comment"]
```

`filenames` is matched case-sensitively and consulted **before** any extension rule, so a claim on
`BUILD.bazel` decides that file regardless of what else claims `.bazel`. Case matters because
`BUILD` and `build` are different files to Bazel, and folding the key would hand a `build` shell
script to a Starlark parser. An entry ending in `.*` claims every name starting with the part before
the `*` — `filenames = ["Dockerfile.*"]` reaches `Dockerfile.prod` — and where two such prefixes
overlap the longer one wins, never whichever was registered first. A `[languages.*]` section needs at
least one `extensions` or `filenames` entry; with neither it claims no file at all and is rejected
rather than silently doing nothing.

</details>

## How It Works

Unlike regex-based tools, uncomment builds a proper Abstract Syntax Tree of your code with
tree-sitter, so it distinguishes:

- Real comments vs comment-like content in strings
- Documentation comments vs regular comments
- Inline comments vs standalone comments
- Language-specific metadata that must be preserved

The pipeline is modular: a **language registry** (51 built-ins, plus any other compiled-in grammar
named in config) feeds an **AST visitor** that finds comment nodes, a **preservation engine** decides
what to keep, and an **output generator** emits clean code.

## Git Hooks

<details>
<summary><b>pre-commit</b></summary>

```yaml
repos:
  - repo: https://github.com/Goldziher/uncomment
    rev: v3.5.0
    hooks:
      - id: uncomment
```

Use `id: uncomment-check` instead to fail the commit on any comment a staged change adds that uncomment would remove, rewriting nothing.

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

To block the commit instead of rewriting the files, check them. `--changed-lines --staged` asks only about the lines the commit changes, so touching a legacy file does not fail on the comments already in it:

```yaml
pre-commit:
  commands:
    uncomment-check:
      run: uncomment --check --staged --changed-lines {staged_files}
```

Drop `--staged --changed-lines` to hold every staged file to the policy in full.

</details>

<details>
<summary><b>GitHub Actions</b></summary>

`--changed-lines` diffs against the merge base, so the checkout needs the base branch's history:

```yaml
jobs:
  uncomment:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - run: cargo install uncomment
      - run: uncomment --check --changed-lines --base origin/${{ github.base_ref }} .
```

On a codebase that already follows the policy, `uncomment --check .` checks everything.

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

*Benchmarked on a large enterprise codebase of ~5,000 mixed-language files.* Measure your own with
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
