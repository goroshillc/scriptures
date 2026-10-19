#!/usr/bin/env bash
# Reproduces the art from its source: every admitted plate's ORIGINAL upload from
# Wikimedia Commons, refused unless its sha1 equals the sha1 Commons recorded for the
# reviewed file (data/art_manifest.json), then resized and re-encoded only (AVIF, WebP,
# JPEG at every width, Lanczos3) into data/art-cache/crisp.
#
# Needs cjpeg (libjpeg-turbo), cwebp (libwebp) and avifenc (libavif) on PATH.
# About 1.5 GB is downloaded, one file at a time with a pause between requests.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
data="$root/crates/scriptures/data"
target="${CARGO_TARGET_DIR:-$root/target}"
threads=${THREADS:-4}
cd "$root"
for tool in cjpeg cwebp avifenc; do
  command -v "$tool" >/dev/null || { echo "missing $tool" >&2; exit 2; }
done
cargo build --release -p goroshi-scriptures
"$target/release/goroshi-scriptures" fetch-originals --data "$data"
"$target/release/goroshi-scriptures" derive-crisp --data "$data" --threads "$threads"
