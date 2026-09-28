#!/usr/bin/env bash
# Builds the uncomment invocation from the action's inputs and runs it, with a problem matcher
# registered around the call so violations land as inline annotations.
set -euo pipefail

# ARGS is a space-separated argument string (e.g. "lint . --fix"); splitting it on whitespace is
# the point, not an accident. Globbing is disabled around it so a glob uncomment itself understands
# (it walks paths/patterns on its own) is not pre-expanded by the shell first.
set -f
# shellcheck disable=SC2206
parsed_args=(${ARGS})
set +f

if [ "$CHANGED_ONLY" = "true" ]; then
  parsed_args+=(--changed-only)
fi
if [ "$CHANGED_LINES" = "true" ]; then
  parsed_args+=(--changed-lines)
fi
if [ "$STAGED" = "true" ]; then
  parsed_args+=(--staged)
fi
if [ -n "$BASE" ]; then
  parsed_args+=(--base "$BASE")
fi

echo "::add-matcher::${MATCHER_PATH}"

cd "$WORKING_DIRECTORY"
echo "Running: uncomment ${parsed_args[*]}"
set +e
uncomment "${parsed_args[@]}"
status=$?
set -e

echo "::remove-matcher owner=uncomment-lint::"
echo "::remove-matcher owner=uncomment-check::"

exit "$status"
