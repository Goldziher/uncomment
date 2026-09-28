#!/usr/bin/env bash
# Downloads and verifies the release asset resolved by gh-action-resolve.sh, unless the
# actions/cache step already restored it. Always exposes the binary on PATH and sets the action's
# `version` / `path` outputs, cache hit or not.
set -euo pipefail

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

bin_name="uncomment"
if [ "$RUNNER_OS" = "Windows" ]; then
  bin_name="uncomment.exe"
fi

if [ "${CACHE_HIT:-}" != "true" ]; then
  mkdir -p "$BIN_DIR"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  (
    cd "$work"

    echo "Downloading ${ASSET} (${TAG}) from ${REPO}..."
    gh release download "$TAG" --repo "$REPO" -p "$ASSET" -p "$CHECKSUMS"

    expected="$(awk -v f="$ASSET" '$2 == f { print $1 }' "$CHECKSUMS")"
    if [ -z "$expected" ]; then
      echo "::error::No checksum entry for ${ASSET} in ${CHECKSUMS}"
      exit 1
    fi

    actual="$(sha256 "$ASSET")"
    if [ "$actual" != "$expected" ]; then
      echo "::error::Checksum mismatch for ${ASSET}: expected ${expected}, got ${actual}"
      exit 1
    fi
    echo "Checksum OK (sha256): ${actual}"

    case "$EXT" in
    tar.gz)
      tar -xzf "$ASSET" -C "$BIN_DIR"
      ;;
    zip)
      if command -v unzip >/dev/null 2>&1; then
        unzip -q -o "$ASSET" -d "$BIN_DIR"
      else
        python3 -c "import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "$ASSET" "$BIN_DIR"
      fi
      ;;
    *)
      echo "::error::Unknown archive extension: ${EXT}"
      exit 1
      ;;
    esac
  )

  if [ "$RUNNER_OS" != "Windows" ]; then
    chmod +x "${BIN_DIR}/${bin_name}"
  fi
else
  echo "uncomment ${TAG} already cached at ${BIN_DIR}"
fi

echo "$BIN_DIR" >>"$GITHUB_PATH"
{
  echo "version=${TAG#v}"
  echo "path=${BIN_DIR}/${bin_name}"
} >>"$GITHUB_OUTPUT"
