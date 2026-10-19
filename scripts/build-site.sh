#!/usr/bin/env bash
# Builds the reader from source into a new directory.
#
#   scripts/build-site.sh web     OUT_DIR   # the website: AVIF, WebP and JPEG at every width
#   scripts/build-site.sh desktop OUT_DIR   # the desktop app's edition: JPEG at three widths
#
# Needs the art in crates/scriptures/data/art-cache/crisp (scripts/fetch-art.sh, or the
# desktop art pack: scripts/art-pack.sh fetch). Then it proves the result: every chapter
# reachable with no broken link (verify) and the search identical in Rust and WebAssembly.
# Environment: SCRIPTURES_BASE_URL (default https://scriptures.goroshillc.com),
# SCRIPTURES_DATE (default today, UTC), CARGO_TARGET_DIR.
set -euo pipefail
edition=${1:?usage: build-site.sh web|desktop OUT_DIR}
out=${2:?usage: build-site.sh web|desktop OUT_DIR}
root=$(cd "$(dirname "$0")/.." && pwd)
data="$root/crates/scriptures/data"
target="${CARGO_TARGET_DIR:-$root/target}"
base="${SCRIPTURES_BASE_URL:-https://scriptures.goroshillc.com}"
date="${SCRIPTURES_DATE:-$(date -u +%F)}"
stamp=$(date -u +%Y%m%d%H%M%S)
cd "$root"
cargo build --release -p goroshi-scriptures
cargo build --release -p scripture-search -p computus --target wasm32-unknown-unknown
cp "$target/wasm32-unknown-unknown/release/computus.wasm" "$data/computus.wasm"
bin="$target/release/goroshi-scriptures"
assets="$target/search-assets-$stamp"
"$bin" bind-search --wasm "$target/wasm32-unknown-unknown/release/scripture_search.wasm" --out "$assets"
"$bin" build --data "$data" --out "$out" --base-url "$base" --search-assets "$assets" \
  --date "$date" --edition "$edition"
"$bin" verify --out "$out" --base-url "$base"
"$bin" check-search-wasm --data "$out" > "$target/search-parity-$stamp.json"
grep -q '"native_wasm_parity": *"PASS"' "$target/search-parity-$stamp.json"
echo "built and verified: $out ($edition edition)"
