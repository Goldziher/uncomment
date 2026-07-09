---
priority: high
---

# Cross-Platform Distribution

uncomment is distributed across multiple package ecosystems. All distribution channels must be kept consistent and functional.

## Distribution Channels

- **crates.io**: Published via `cargo publish`. Primary source of truth for the Rust binary.
- **npm** (package: `uncomment-cli`): Binary wrapper in `npm-package/` that downloads pre-built binaries from GitHub Releases via `install.js`.
- **PyPI** (package: `uncomment`): Binary wrapper in `pip-package/` with `uncomment/downloader.py` handling binary acquisition.
- **Homebrew**: Formula in the `Goldziher/homebrew-tap` repo (`Formula/uncomment.rb`), regenerated from the release checksums by `scripts/update-homebrew-formula.sh` and pushed by the `publish_homebrew` job.
- **GitHub Actions**: Cross-platform binary builds and release automation live in `.github/workflows/publish.yaml` (a native per-platform build matrix — goreleaser was removed in v3.1.0).

## Release Workflow

1. Update version numbers in all manifests (see version-synchronization rule)
2. Commit and tag with `vX.Y.Z`
3. Push the tag to trigger `.github/workflows/publish.yaml`
4. The workflow creates a draft release, builds binaries for all platforms natively
   (macOS on `macos-latest`/`macos-15-intel`, Windows cross-compiled with mingw-w64,
   Linux in `manylinux_2_28` containers), uploads them plus a checksums file, then
   promotes the draft to a published release
5. npm, PyPI, and Homebrew publish from the finalized GitHub Release assets

## Guidelines

- Always test binary wrapper install scripts when modifying `npm-package/install.js` or `pip-package/uncomment/downloader.py`
- Ensure binaries are built for: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-gnu`
- Tree-sitter grammars are downloaded on demand at runtime (tree-sitter-language-pack dynamic mode); they are not baked into the binary
- RC releases use `vX.Y.Z-rc.N` tags (note: PyPI uses `X.Y.ZrcN` format without hyphens)
