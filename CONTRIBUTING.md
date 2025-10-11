# Contributing to Uncomment

Thanks for helping improve `uncomment`! This guide keeps the local workflow and release process clear and to the point.

## Local Development

### Prerequisites

- Rust 1.70+ with `cargo`
- Node.js 18+ (for optional tooling and npm wrapper verification)
- `tree-sitter` grammars are bundled; no extra setup is required

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

### Automation Hooks (Prek)

We use [prek](https://github.com/nikitonsky/prek), a Rust rewrite of `pre-commit`, to execute the hooks defined in `.pre-commit-config.yaml`. It is a drop-in replacement that runs the same checks faster.

```bash
cargo install prek            # or use your preferred binary installer
prek install
prek install --hook-type commit-msg
prek run --all-files          # run hooks on demand
```

## Distribution & Releases

`uncomment` ships through four channels: crates.io, npm, PyPI, and GitHub Releases. Keep versions in sync across all artifacts.

### Version Bump Checklist

1. Update versions:
   - `Cargo.toml`
   - `npm-package/package.json`
   - `pip-package/pyproject.toml`
   - `pip-package/uncomment/__init__.py`
2. Regenerate `Cargo.lock` if needed (`cargo update`).
3. Commit the changes with a descriptive message.

### Tagging

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Release candidates use `vX.Y.Z-rc.1` for Git/npm and `X.Y.Zrc1` for PyPI.

### Publishing

- **Cargo**: `cargo publish`
- **npm**: `cd npm-package && npm publish` (use `--tag beta` for RCs)
- **PyPI**:
  ```bash
  cd pip-package
  python -m build
  twine upload dist/*          # use --repository testpypi for RCs
  ```

### GitHub Release Workflow

Pushing the tag triggers `.github/workflows/release.yml`, which builds signed binaries for macOS (Intel/Apple Silicon), Linux (x86_64/aarch64), and Windows, attaches them to the GitHub release, and makes them available to the npm and PyPI shims.

### After Publishing

- Verify installs: `cargo install uncomment`, `npm view uncomment-cli`, `pip index versions uncomment`
- Announce the release and update any downstream docs if needed
