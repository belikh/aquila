#!/bin/sh
# Externalise Trunk's inline module bootstrap so the strict ingress CSP
# (`script-src 'self' 'wasm-unsafe-eval'`, `dashboard.md` §8) can boot the SPA.
#
# Trunk injects a `<script type="module">…</script>` block into `index.html`. An
# inline script is blocked by `script-src 'self'`; moving it to a same-origin
# external file keeps the policy strict with no `'unsafe-inline'` and no
# per-build hash. Run as a Trunk `post_build` hook; `TRUNK_STAGING_DIR` is the
# build output directory.
set -eu

dir="${TRUNK_STAGING_DIR:?TRUNK_STAGING_DIR must be set by Trunk}"
html="$dir/index.html"

if [ ! -f "$html" ]; then
    echo "csp hook: $html not found" >&2
    exit 1
fi

init="$dir/aquila-init.js"
awk -v init="$init" '
    /<script type="module">/ {
        print "<script type=\"module\" src=\"./aquila-init.js\"></script>"
        capturing = 1
        next
    }
    capturing && /<\/script>/ {
        capturing = 0
        next
    }
    capturing { print > init; next }
    { print }
' "$html" > "$html.tmp"
mv "$html.tmp" "$html"
