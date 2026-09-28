#!/usr/bin/env bash
# Resolves the release tag and the release asset for this runner, and writes both — plus the
# directory the binary will be cached under — to $GITHUB_OUTPUT. Split out from install so the
# install directory is known before the actions/cache step that wraps it.
set -euo pipefail

repo="${REPOSITORY:-Goldziher/uncomment}"

version="${VERSION_INPUT:-}"
if [ -z "$version" ]; then
  # A version tag such as `v3.10.0` when the action is pinned to a release; anything else
  # (a branch, a SHA, a local `./` checkout) cannot be mapped to a release, so fall back.
  if [[ "$ACTION_REF" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+([+.-][0-9A-Za-z.]+)?$ ]]; then
    version="$ACTION_REF"
  else
    version="latest"
  fi
fi

if [ "$version" = "latest" ]; then
  tag="$(gh api "repos/${repo}/releases/latest" -q .tag_name)"
else
  case "$version" in
    v*) tag="$version" ;;
    *) tag="v${version}" ;;
  esac
fi

case "$RUNNER_OS" in
  Linux)
    case "$RUNNER_ARCH" in
      X64) triple="x86_64-unknown-linux-gnu" ;;
      ARM64) triple="aarch64-unknown-linux-gnu" ;;
      *)
        echo "::error::uncomment publishes no Linux release asset for architecture $RUNNER_ARCH"
        exit 1
        ;;
    esac
    ext="tar.gz"
    ;;
  macOS)
    case "$RUNNER_ARCH" in
      ARM64) triple="aarch64-apple-darwin" ;;
      X64) triple="x86_64-apple-darwin" ;;
      *)
        echo "::error::uncomment publishes no macOS release asset for architecture $RUNNER_ARCH"
        exit 1
        ;;
    esac
    ext="tar.gz"
    ;;
  Windows)
    case "$RUNNER_ARCH" in
      X64) triple="x86_64-pc-windows-gnu" ;;
      *)
        echo "::error::uncomment publishes no Windows release asset for architecture $RUNNER_ARCH"
        exit 1
        ;;
    esac
    ext="zip"
    ;;
  *)
    echo "::error::Unsupported runner OS: $RUNNER_OS"
    exit 1
    ;;
esac

asset="uncomment-${triple}.${ext}"
bare_version="${tag#v}"
checksums="uncomment_${bare_version}_checksums.txt"
install_dir="${RUNNER_TEMP}/uncomment-action/${tag}-${RUNNER_OS}-${RUNNER_ARCH}"
bin_dir="${install_dir}/bin"

{
  echo "repo=${repo}"
  echo "tag=${tag}"
  echo "asset=${asset}"
  echo "ext=${ext}"
  echo "checksums=${checksums}"
  echo "bin-dir=${bin_dir}"
} >>"$GITHUB_OUTPUT"
