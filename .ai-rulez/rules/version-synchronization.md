---
priority: high
---

# Version Synchronization

All version numbers must be kept in sync across the following files when releasing:

## Version Locations

1. `Cargo.toml` — the `version` field under `[package]`
2. `npm-package/package.json` — the `version` field
3. `pip-package/pyproject.toml` — the `version` field under `[project]`
4. `pip-package/uncomment/__init__.py` — the `__version__` variable

## Rules

- All four files must contain the same version string when preparing a release
- Use semantic versioning (SemVer): `MAJOR.MINOR.PATCH`
- When bumping versions, update all four files in a single commit
- The git tag format is `vX.Y.Z` (e.g., `v2.5.1`)
- For release candidates: Cargo/npm use `X.Y.Z-rc.N`, PyPI uses `X.Y.ZrcN`
- Never create a release tag without first verifying all versions match
