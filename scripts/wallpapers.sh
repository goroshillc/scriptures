#!/usr/bin/env bash
# The Doré wallpaper pack: every admitted plate centred on white at common screen sizes,
# one folder and one zip per size, numbered in the order of the Bible.
#
#   scripts/wallpapers.sh OUT_DIR [--sizes 1920x1080,3840x2160]
#
# Needs the Commons originals in crates/scriptures/data/art-cache/originals
# (`goroshi-scriptures fetch-originals`, part of scripts/fetch-art.sh) and cjpeg.
set -euo pipefail
out=${1:?usage: wallpapers.sh OUT_DIR [--sizes WxH,...]}
shift
root=$(cd "$(dirname "$0")/.." && pwd)
target="${CARGO_TARGET_DIR:-$root/target}"
cd "$root"
cargo build --release -p goroshi-scriptures
"$target/release/goroshi-scriptures" wallpapers --data "$root/crates/scriptures/data" \
  --out "$out" --threads "${THREADS:-4}" "$@"
