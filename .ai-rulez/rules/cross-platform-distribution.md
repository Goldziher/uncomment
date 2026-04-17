---
priority: high
---

# Cross-Platform Distribution

uncomment is distributed across multiple package ecosystems. All distribution channels must be kept consistent and functional.

## Distribution Channels

- **crates.io**: Published via `cargo publish`. Primary source of truth for the Rust binary.
- **npm** (package: `uncomment-cli`): Binary wrapper in `npm-package/` that downloads pre-built binaries from GitHub Releases via `install.js`.
- **PyPI** (package: `uncomment`): Binary wrapper in `pip-package/` with `uncomment/downloader.py` handling binary acquisition.
- **Homebrew**: Formula in `homebrew-tap/Formula/uncomment.rb`, auto-updated via `mislav/bump-homebrew-formula-action` on release.
- **goreleaser**: Cross-compilation and release automation via `.goreleaser.yaml`.

## Release Workflow

1. Update version numbers in all manifests (see version-synchronization rule)
2. Commit and tag with `vX.Y.Z`
3. Push tag to trigger `.github/workflows/release-homebrew.yml`
4. GitHub Actions builds binaries for all platforms and creates a GitHub Release
5. npm and PyPI wrappers download from the GitHub Release assets

## Guidelines

- Always test binary wrapper install scripts when modifying `npm-package/install.js` or `pip-package/uncomment/downloader.py`
- Ensure binaries are built for: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`
- RC releases use `vX.Y.Z-rc.N` tags (note: PyPI uses `X.Y.ZrcN` format without hyphens)
