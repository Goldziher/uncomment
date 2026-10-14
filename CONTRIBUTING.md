# Contributing to Uncomment

Thanks for helping improve `uncomment`! This guide keeps the local workflow and release process clear and to the point.

## Local Development

### Prerequisites

- Rust 1.90+ with `cargo`
- Python 3.11+ for governance hooks and plugin checks
- Poly and ai-rulez 5.3.0 for governance generation
- Node.js 24+ (for optional tooling and npm wrapper verification)
- Tree-sitter grammars are provided by [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack); no extra setup is required

### Bootstrap

```bash
git clone https://github.com/Goldziher/uncomment.git
cd uncomment
cargo check
cargo test
```

Run the usual hygiene commands before sending a change:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- --deny warnings
cargo test --all-features
```

## Pre-commit hooks

Install the git hooks with `task setup` (or `poly hooks install` directly). On
every commit, poly runs lint, format, and file-safety checks plus `cargo clippy`;
the commit-msg hook validates the message. Run all hooks manually with
`poly hooks run pre-commit --all-files`.

## Distribution & Releases

`uncomment` ships through four channels: crates.io, npm, PyPI, and GitHub Releases. Keep versions in sync across all artifacts.

### Version Bump Checklist

1. Update versions:
   - `Cargo.toml`
   - `npm-package/package.json`
   - `pip-package/pyproject.toml`
   - `pip-package/uncomment/__init__.py`
2. Run `task plugin:generate` to synchronize the plugin version and public skills.
3. Regenerate `Cargo.lock` if needed (`cargo update`).
4. Commit the changes with a descriptive message.

### Tagging

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Release candidates use `vX.Y.Z-rc.1` for Git/npm and `X.Y.Zrc1` for PyPI.

### Publishing

- **Cargo**: `cargo publish`
- **npm**: `cd npm-package && pnpm publish` (use `--tag beta` for RCs)
- **PyPI**:

  ```bash
  cd pip-package
  python -m build
  twine upload dist/*          # use --repository testpypi for RCs
  ```

### GitHub Release Workflow

Pushing the tag triggers `.github/workflows/publish.yaml`, which builds signed binaries for macOS (Intel/Apple Silicon), Linux (x86_64/aarch64), and Windows, attaches them to the GitHub release, and makes them available to the npm and PyPI shims.

### After Publishing

- Verify installs: `cargo install uncomment`, `pnpm view uncomment-cli`, `pip index versions uncomment`
- Announce the release and update any downstream docs if needed

## AI governance and public plugins

Edit `.ai-rulez/` for repository instructions and `plugin/.ai-rulez/` for public
skills. Both use OKF content and native schema version `5.0`; the CLI is 5.3.0.
The plugin has no MCP server and requires a separately installed Uncomment CLI.

Run `task ai:hooks` to initialize system-channel hook preferences only when no
local preferences file exists, refresh the pinned hook catalog, and install hooks.
Run `task plugin:generate` after editing skills or bumping the CLI version, then
`task ai:validate`. Generated runtime bundles and the portable `skills/` export
must remain byte-for-byte current. Public skills are copied only from native
Agent Skills output; ownership checks preserve unowned files and reject symlinks.

The release workflow packages Agent Plugins archives and a Cursor team marketplace,
verifies checksums, and uploads them before the release can be promoted. Plugin
versions follow `[package].version` in `Cargo.toml`; release tags must agree.
