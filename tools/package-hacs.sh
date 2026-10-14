#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Aquila contributors
# SPDX-License-Identifier: GPL-3.0-only
#
# Package the HACS integration as a release asset (home-assistant.md §3.3,
# repository-tooling.md §10.4, §12).
#
# HACS `zip_release` expects a single asset named `aquila.zip` whose archive
# contains `custom_components/aquila/` (the repository path is independent of the
# HA runtime path). This script stages the component's *runtime* files only —
# no tests, no `__pycache__`, no `.pyc` — and zips them deterministically:
# fixed file order (LC_ALL=C sort) and a fixed mtime, so the same commit yields
# the same bytes.
#
# Usage: tools/package-hacs.sh [OUTPUT_ZIP]
#   OUTPUT_ZIP defaults to <repo-root>/aquila.zip.
#
# Requires: git (to find the repo root; optional), coreutils (find, sort, touch),
# and zip.

set -eu

root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
out=${1:-"$root/aquila.zip"}
component="$root/custom_components/aquila"

if [ ! -d "$component" ]; then
    echo "package-hacs: component not found: $component" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM

mkdir -p "$work/custom_components"
cp -R "$component" "$work/custom_components/aquila"

# Drop dev-only and generated files from the shipped integration.
rm -rf "$work/custom_components/aquila/tests" \
       "$work/custom_components/aquila/__pycache__" \
       "$work/custom_components/aquila/.pytest_cache"
find "$work/custom_components/aquila" -name '__pycache__' -type d -prune -exec rm -rf {} +
find "$work/custom_components/aquila" -name '.pytest_cache' -type d -prune -exec rm -rf {} +
find "$work/custom_components/aquila" -name '*.pyc' -delete

# Normalise timestamps for a reproducible archive.
find "$work/custom_components/aquila" -exec touch -t 198001010000.00 {} +

files=$(find "$work/custom_components/aquila" -type f | wc -l | tr -d ' ')

rm -f "$out"
( cd "$work" && find custom_components/aquila -type f -print \
    | LC_ALL=C sort \
    | zip -X -q -9 "$out" -@ )

echo "package-hacs: wrote $out ($files files)"
