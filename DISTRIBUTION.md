# Distribution Guide

This document explains how to distribute `uncomment` across multiple package managers.

## Current Distribution Channels

1. **Cargo (Rust)** ✅ - Already published at https://crates.io/crates/uncomment
2. **npm (Node.js)** 🔨 - Package ready in `npm-package/`
3. **pip (Python)** 🔨 - Package ready in `pip-package/`
4. **GitHub Releases** 🔨 - Cross-platform binaries via GitHub Actions

## NPM Package Structure

Located in `npm-package/`:

- Uses `binary-install` to download platform-specific binaries
- Automatically detects platform (Windows, macOS, Linux) and architecture
- Downloads from GitHub Releases at install time
- Provides `uncomment` command globally after `npm install -g uncomment`

### Publishing to npm

```bash
cd npm-package
# For release candidates
npm publish --tag beta

# For stable releases
npm publish
```

## Python Package Structure

Located in `pip-package/`:

- Uses modern `pyproject.toml` configuration
- Downloads platform-specific binaries on first run
- Caches binaries in `~/.cache/uncomment/`
- Provides `uncomment` command after `pip install uncomment`

### Publishing to PyPI

```bash
cd pip-package
pip install build twine

# Build the package
python -m build

# For release candidates (TestPyPI)
twine upload --repository testpypi dist/*

# For stable releases (PyPI)
twine upload dist/*
```

## GitHub Actions Workflow

The `.github/workflows/release.yml` builds cross-platform binaries:

- **Linux**: x86_64, aarch64 (ARM64)
- **Windows**: x86_64, i686 (32-bit)
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)

### Triggering a Release

1. Create and push a tag: `git tag v2.1.1 && git push origin v2.1.1`
2. GitHub Actions automatically builds binaries for all platforms
3. Creates a GitHub Release with all binaries attached
4. npm and pip packages can then download these binaries

## Installation Methods

After publishing, users can install via:

```bash
# Rust ecosystem
cargo install uncomment

# Node.js ecosystem
npm install -g uncomment

# Python ecosystem
pip install uncomment

# Direct download
# Download from GitHub Releases page
```

## Version Synchronization

Keep versions synchronized across all packages:

### For Release Candidates (Testing)

1. Update `Cargo.toml` version (e.g., `2.1.1-rc.1`)
2. Update `npm-package/package.json` version (e.g., `2.1.1-rc.1`)
3. Update `pip-package/pyproject.toml` version (e.g., `2.1.1rc1`)
4. Update `pip-package/uncomment/__init__.py` version (e.g., `2.1.1rc1`)
5. Create and push git tag: `git tag v2.1.1-rc.1`
6. Test with pre-release publications

### For Final Release

1. Remove `-rc.X` suffix from all versions
2. Create and push final tag: `git tag v2.1.1`
3. Publish to cargo, npm, and PyPI

**Note**: Python uses `rc1` format while npm/git use `-rc.1` format per their conventions.

## Benefits

- **No compilation required**: Users don't need Rust toolchain
- **Familiar package managers**: Developers can use their preferred ecosystem
- **Cross-platform**: Automatic platform detection and binary selection
- **Fast installation**: Pre-compiled binaries, no build time
- **Consistent experience**: Same CLI across all installation methods
