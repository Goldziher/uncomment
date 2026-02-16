# Changelog

All notable changes to this project are documented in this file.

This changelog is generated from git tags and commit history.

## [Unreleased]

- _No changes yet._

## [v2.11.0] - 2026-02-16

### Added

- Built-in language support for Haskell, HTML, CSS, XML, SQL, Kotlin, Swift, Lua, Nix, PowerShell, Protocol Buffers, and INI.
- Language fixtures and coverage for the new built-in grammars.

### Changed

- Updated tree-sitter dependencies and static grammar registrations for expanded built-in language support.
- Expanded language registry mappings and smart-init extension detection for newly supported languages.
- Updated tests to reflect built-in grammar behavior for languages that no longer require dynamic grammar configuration.

### Documentation

- Updated README language coverage and dynamic grammar examples to reflect the current built-in language set.

## [v2.10.4] - 2026-01-08

### Changed

- Updated Rust dependencies.

## [v2.10.3] - 2025-12-13

### Changed

- Switched crates.io publishing in CI to Trusted Publishing (OIDC), removing the need for a long-lived `CARGO_TOKEN`.
- Windows release assets now target `x86_64-pc-windows-gnu` only (32-bit Windows is no longer supported by the binary wrappers).

## [v2.10.2] - 2025-12-13

### Changed

- Aligned publishing workflows with `gitfluff`: GoReleaser-built GitHub release assets plus split registry publishing jobs (crates.io, npm, PyPI).
- Publishing is now idempotent: if a version is already published, CI skips re-publishing instead of failing.
- Windows release assets are now published as `.zip` archives (Linux/macOS remain `.tar.gz`).

### Fixed

- Python wrapper now downloads binaries without the `requests` dependency and caches per-version; use `UNCOMMENT_BINARY` to override the binary path.

## [v2.10.1] - 2025-12-13

### Fixed

- Preserve shebang lines (e.g. `#!/usr/bin/env bash`) even when not on the first line, and even when `--no-default-ignores` is used
- Avoid broken pipe panics when piping output (e.g. `uncomment ... | head`)
- Preserve common auto-generated / do-not-edit file header comments
- Preserve C/C++ preprocessor trailing comments (e.g. `#endif /* HEADER_GUARD */`)

### Changed

- Summarize unsupported files once (instead of printing per-file errors), with examples in `--verbose`
- Make dry-run output quiet by default; add `--diff` to show line-by-line diffs
- Detect and warn about potentially important comment removals (with examples in `--verbose`)

## [v2.10.0] - 2025-12-12

### Added

- Built-in language support: Ruby, PHP, Elixir, TOML, C#

### Fixed

- Preserve Go embed/cgo directives and cgo preambles
- Preserve Ruby magic comments and YARD docs by default

### Changed

- Supported file detection now follows the language registry

### Documentation

- Added `CHANGELOG.md`

## [v2.9.2] - 2025-12-01

### Chore

- chore: bump version to 2.9.2 (b559f01)

## [v2.9.1] - 2025-12-01

### Chore

- chore: bump version to 2.9.1 (d0aec0b)
- chore: updated deps (c85ceb3)

### Other

- ci(deps)(deps): bump actions/checkout from 5 to 6 (b6ca642)
- build(deps)(deps): bump clap in the production-dependencies group (fb0565f)
- ci(deps)(deps): bump actions/setup-node from 5 to 6 (3020595)

## [v2.9.0] - 2025-11-12

### Chore

- chore: migrate to saphyr and update dependencies (0090245)
- chore: align versions to 2.9.0 and clean clippy (b1989fe)

### Other

- ci(deps)(deps): bump actions/download-artifact from 5 to 6 (dad93d1)
- ci(deps)(deps): bump actions/upload-artifact from 4 to 5 (f6ea7a6)

## [v2.8.3] - 2025-10-11

### Fixed

- fix: preserve rust attribute macros when removing doc comments (279d212)

### Chore

- chore: run prek hooks (33c8744)
- chore: bump version to 2.8.3 (ab84add)
- chore: strip redundant comments (d2dcdcf)

## [v2.8.2] - 2025-10-11

### Chore

- chore: bump version to 2.8.2 (b3f6c72)

## [v2.8.1] - 2025-10-11

### Fixed

- fix: resolve index out of bounds panic and bump to v2.8.1 (772606f)

## [v2.8.0] - 2025-10-10

### Fixed

- fix: gate benchmarking binaries behind feature (0871d7a)

### Documentation

- docs: consolidate contributor and release guidance (4425df4)

### Chore

- chore: bump to v2.8.0 and cancel stale workflows (b2ba2aa)
- chore: cleanup gitignored detritus (eeacfea)
- chore: updated dependencies and added ai-rulez (d4f35d9)

### Other

- build(deps)(deps): bump the production-dependencies group with 5 updates (e5c3159)
- build(deps)(deps): bump serde in the production-dependencies group (05fb237)
- build(deps)(deps): bump tree-sitter-python in the tree-sitter group (ed1105f)

## [v2.7.0] - 2025-09-11

### Added

- feat: enhance linting tool comment preservation for all languages (5e9ed38)

### Fixed

- fix: ignore flaky network-dependent integration test in CI (f43c133)

### Changed

- refactor: cleanup and reorganize repository structure (f992978)

### Documentation

- docs: add GitHub Sponsors button to README (aa85949)

### Chore

- chore: bump version to v2.7.0 (fa1d016)

### Other

- ci(deps)(deps): bump actions/setup-python from 5 to 6 (9925521)
- ci(deps)(deps): bump actions/setup-node from 4 to 5 (c15f1f8)
- build(deps)(deps): bump clap in the production-dependencies group (041dac9)
- build(deps)(deps): bump the tree-sitter group with 4 updates (786574d)
- build(deps)(deps): bump regex in the production-dependencies group (e63598b)

## [v2.6.0] - 2025-08-23

### Added

- feat: implement CLI flag fixes and code cleanup for v2.6.0 (5e80837)

### Build

- ci: re-enable automatic package publishing on release (7f3f416)

### Other

- ci(deps)(deps): bump amannn/action-semantic-pull-request from 5 to 6 (b059c01)
- ci(deps)(deps): bump actions/checkout from 4 to 5 (b0a7dff)

## [v2.5.0] - 2025-08-13

### Fixed

- fix: update print statements for grammar cloning and compilation (6d81a6d)
- fix: correct output formatting for detected languages in CLI (1422456)

### Changed

- refactor: improve formatting and readability in integration test (274d74f)
- refactor: improve comment handling in processor (b326360)

### Testing

- test: add integration test for uncommenting code in multiple repositories (0fb3c6d)

### Chore

- chore: update repos.yaml by removing outdated repository URLs (f4fa8d5)
- chore: add formatting check to CI workflow (c9e0640)
- chore: update repos.yaml with additional repository URLs for integration testing (77a8fee)
- chore: add integration test repositories configuration (bd2f7da)
- chore: add serde_yaml dependency to Cargo.toml (5db6ec1)
- chore: update Cargo.lock to include serde_yaml and unsafe-libyaml dependencies (5897f55)
- chore: update .gitignore to include integration test repos cache directory (ebdcb46)

### Other

- Update all package versions to 2.5.0 and document Go documentation comment detection (8bbf5e4)
- Bump version to 2.5.0 (a5d2001)
- Implement Go documentation comment detection with extensible language handler architecture (69e8d8d)
- ci(deps)(deps): bump actions/download-artifact from 4 to 5 (90cdcf9)
- build(deps)(deps): bump the production-dependencies group with 3 updates (dc30793)
- build(deps)(deps): bump the production-dependencies group with 2 updates (244b25b)
- build(deps)(deps): bump the tree-sitter group with 2 updates (02ce191)
- build(deps)(deps): bump the production-dependencies group with 2 updates (1ce3678)
- build(deps)(deps): bump dirs from 5.0.1 to 6.0.0 (4343eed)

## [v2.4.2] - 2025-07-01

### Documentation

- docs: update README and CLAUDE.md with Homebrew installation (ae43f31)

## [v2.4.1] - 2025-07-01

### Added

- feat: bump version to v2.4.1 and finalize homebrew release pipeline (d77a20e)
- feat: update homebrew-tap submodule to v2.4.1-rc.3 (ca1a9ba)
- feat: implement Homebrew release pipeline (f3138d4)

### Fixed

- fix: handle existing releases in homebrew workflow (5d80c93)
- fix: resolve clippy::uninlined_format_args warnings for Rust 1.88 (2371d2f)

### Changed

- refactor: simplify Homebrew workflow to use source-based builds (cc5a4d9)

### Documentation

- docs: update README and CLAUDE.md with Homebrew installation (85dcfd9)

## [v2.4.0] - 2025-07-01

### Added

- feat: implement intelligent init command with tree-sitter grammar integration (7eb8130)
- feat: implement dynamic tree-sitter grammar loading system (7b1ba6c)
- feat: implement comprehensive TOML configuration system (269be1b)
- feat: add manual trigger support to publish workflow (19eaea3)

### Fixed

- fix: clippy uninlined format args warnings in config.rs (90acca0)
- fix: language-specific configuration for python docstrings (d1cffd2)
- fix: ensure `--version` matches what is in `Cargo.toml` (b16a09e)

### Documentation

- docs: update README and CLAUDE.md for v2.4.0 features (62ce35a)

### Chore

- chore: bump version to 2.4.0 (09ffa5c)
- chore: removed claude code review (9d9f9a9)

## [v2.3.1] - 2025-06-29

### Fixed

- fix: remove Zig dependency to enable crates.io publishing (v2.3.1) (b856b18)

## [v2.3.0] - 2025-06-29

### Added

- feat: add Cargo.lock to repository for Nix compatibility (6d71328)
- feat: add rc files support for shell/bash (e3568dd)
- feat: add haskell language support (3460452)
- feat: add shell/bash language support (847e0fb)
- feat: add zig language support (9fa1416)

### Fixed

- fix: respect parent .gitignore files when running from subdirectories (18fe916)
- fix: rebase and fix conflicts (079c0db)
- fix: rebase and fix confilcts (66d372f)
- fix: apply clippy fixes (d709cda)
- fix: apply clippy suggestions (94d0039)

### Chore

- chore: organise cargo imports (f892ac3)
- chore: organise cargo imports (752ade3)

### Other

- Claude Code Review workflow (30c7217)
- Claude PR Assistant workflow (1b55fca)
- doc: update `README.md` (36c8821)
- doc: update `README.md` (f20719e)
- doc: update `README.md` (8e207e4)

## [v2.2.3] - 2025-06-25

### Chore

- chore: bump version to 2.2.3 (409208f)

## [v2.2.2] - 2025-06-25

### Fixed

- fix: remove version suffix from release asset names to match download scripts (1593d37)

### Chore

- chore: bump version to 2.2.2 (030f519)

## [v2.2.1] - 2025-06-25

### Added

- feat: re-introduce pre-commit hooks with pip installation and git hooks documentation (a9183e1)

### Fixed

- fix: improve gitignore handling and fix npm/pypi download URLs (be0ff73)

### Documentation

- docs: update CLAUDE.md with multi-platform distribution learnings (0035813)

### Chore

- chore: applied pre-commit (5c37b4c)

## [v2.2.0] - 2025-06-25

### Added

- feat: change npm package name to uncomment-cli and update docs (1a61419)

### Fixed

- fix: handle case where npm version is already set in publish workflow (1bce920)

### Documentation

- docs: improve package descriptions and READMEs for npm and PyPI (225cc08)

### Chore

- chore: bump version to 2.2.0 for release (a668a18)
- chore: bump version to 2.1.1 and add .npmignore (85d100a)

## [v2.1.1-rc.7] - 2025-06-25

### Added

- feat: implement comprehensive package distribution system (5f79d22)

### Fixed

- fix: add missing fi in PyPI publish script (f528e4b)
- fix: change npm package name to uncomment-ast (be9bd21)
- fix: update npm install command in publish workflow (2bdb2a7)
- fix: change npm package name to @goldziher/uncomment to avoid naming conflict (7da9581)
- fix: add --allow-dirty flag to cargo publish command (b7e884c)

## [v2.1.1-rc.1] - 2025-06-24

### Added

- feat: add npm and pip package distribution with RC versioning (bd2668a)

### Fixed

- fix: align Python package version format with git tags (46feeca)

### Documentation

- docs: update README with v2.1.0 features and performance benchmarks (82739f1)

## [v2.1.0] - 2025-06-24

### Added

- feat: add benchmarking and profiling tools (f74d55a)
- feat: add support for YAML, HCL/Terraform, and Makefile languages (2ef8ec0)
- feat: expand file extension support for Python and TypeScript variants (c6cbcbe)
- feat: update all dependencies to latest versions and add parallel processing (d9db791)
- feat: enhance linting directive preservation and git repository handling (e69bf97)

### Documentation

- docs: update CLAUDE.md to reflect tree-sitter rewrite (79c045d)

### Chore

- chore: bump version to 2.1.0 (4057420)

## [v2.0.0] - 2025-06-23

### Added

- feat: complete tree-sitter based rewrite with enhanced language support (34d9d23)
- feat: Add recursive .gitignore parsing to uncomment tool (73c8b98)

### Changed

- refactor: remove dead code and simplify preservation rules (b6801ac)

### Documentation

- docs: add CLAUDE.md for AI assistant context (4f10076)

### Chore

- chore: applied formatting (2f47cfb)

### Other

- Fix: Resolve TypeScript regex mangling issue (#8) (d8bc465)

## [v1.0.5] - 2025-04-16

### Added

- feat: v1.0.5 (7f030f6)

### Chore

- chore: fixed doc string mangling (821ce0e)
- chore: fixed issues with typescript files (79aaf07)
- chore: removed comments (2839569)
- chore: fixed test mangling of rust code (8dd6641)
- chore: refactored codebase (8dbc343)
- chore: updated deps (70fb7c1)
- chore: add failing test (daaaa95)
- chore: Update README.md (8a9fe9f)

## [v1.0.4] - 2025-03-08

### Chore

- chore: fixed exit codes (8257feb)

## [v1.0.3] - 2025-03-08

### Added

- feat: v1.0.3 (5ecd708)

### Chore

- chore: fixed doc string removal (d617646)
- chore: switched to using regex (86f82b5)

## [v1.0.2] - 2025-03-08

### Chore

- chore: fix mangling issues (810dcf9)

## [v1.0.1] - 2025-03-08

### Chore

- chore: updated pre-commit hook (c222f35)
- chore: fix unique errors (417d503)
- chore: updated shell script (a4b7b91)
- chore: added installation script and pre-commit hooks (93d528c)

### Other

- Update readme.md (1aadf3f)

## [v1.0.0] - 2025-03-08

### Chore

- chore: downgraded edition (60fdda3)
- chore: added github workflows (9d243e4)
- chore: updated rust edition (2019bfb)

### Other

- initial (f4c7969)
