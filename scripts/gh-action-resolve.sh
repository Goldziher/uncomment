#!/usr/bin/env bash
# Resolves the release tag and the release asset for this runner, and writes both — plus the
# directory the binary will be cached under — to $GITHUB_OUTPUT. Split out from install so the
# install directory is known before the actions/cache step that wraps it.
set -euo pipefail

repo="${REPOSITORY:-Goldziher/uncomment}"

# Resolve a floating `latest` or major-only `v3` reference to the newest matching stable release.
# A floating major tag carries no release assets of its own, so the assets always come from the
# newest full `vN.x.y` release in that series. The releases API is newest-first; drafts,
# pre-releases and the major-only tag itself are skipped.
newest_release() {
  local major="${1:-}" pattern
  if [ -n "$major" ]; then
    pattern="^v${major}\\.[0-9]+\\.[0-9]+$"
  else
    pattern='^v[0-9]+\.[0-9]+\.[0-9]+$'
  fi
  gh api "repos/${repo}/releases?per_page=100" \
    --jq '.[] | select(.draft | not) | select(.prerelease | not) | .tag_name' |
    grep -E "$pattern" | head -n 1
}

version="${VERSION_INPUT:-}"
if [ -z "$version" ]; then
  # A full release tag (`v3.10.0`) or a floating major tag (`v3`) when the action is pinned to
  # one; anything else (a branch, a SHA, a local `./` checkout) falls back to the newest release.
  if [[ "$ACTION_REF" =~ ^v?[0-9]+(\.[0-9]+\.[0-9]+([+.-][0-9A-Za-z.]+)?)?$ ]]; then
    version="$ACTION_REF"
  else
    version="latest"
  fi
fi

if [ "$version" = "latest" ]; then
  tag="$(newest_release)"
elif [[ "$version" =~ ^v?[0-9]+$ ]]; then
  tag="$(newest_release "${version#v}")"
else
  case "$version" in
  v*) tag="$version" ;;
  *) tag="v${version}" ;;
  esac
fi

if [ -z "$tag" ]; then
  echo "::error::No published release found for version '${version}' in ${repo}"
  exit 1
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
