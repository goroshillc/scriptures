#!/usr/bin/env bash
# The desktop art pack: the JPEGs the desktop edition uses, so the app can be built
# without re-deriving 217 plates from their Commons originals.
#
#   scripts/art-pack.sh fetch          # download the pinned pack, check its sha256, unpack
#   scripts/art-pack.sh make OUT.tar   # maintainers: pack data/art-cache/crisp (after fetch-art.sh)
#
# The pack's URL and sha256 are pinned in crates/scriptures/data/art-pack.txt.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
data="$root/crates/scriptures/data"
target="${CARGO_TARGET_DIR:-$root/target}"
pin="$data/art-pack.txt"
case "${1:-}" in
  fetch)
    url=$(awk '$1 == "url" {print $2}' "$pin")
    sum=$(awk '$1 == "sha256" {print $2}' "$pin")
    tmp="$target/art-pack-download"
    mkdir -p "$tmp" "$data/art-cache"
    curl -fL --retry 3 -o "$tmp/pack.tar" "$url"
    got=$( (command -v sha256sum >/dev/null && sha256sum "$tmp/pack.tar" || shasum -a 256 "$tmp/pack.tar") | awk '{print $1}')
    [[ "$got" == "$sum" ]] || { echo "art pack sha256 $got, expected $sum: refusing" >&2; exit 1; }
    tar -xf "$tmp/pack.tar" -C "$data/art-cache"
    echo "art pack verified ($sum) and unpacked into $data/art-cache/crisp"
    ;;
  make)
    out=${2:?usage: art-pack.sh make OUT.tar}
    [[ -e "$out" ]] && { echo "$out exists; choose a new name" >&2; exit 2; }
    cd "$root"
    cargo build --release -p goroshi-scriptures
    list="$target/art-pack-files.txt"
    "$target/release/goroshi-scriptures" desktop-files --data "$data" > "$list"
    # No owner names or macOS metadata in the archive: numeric root ownership only.
    if tar --version 2>/dev/null | grep -q bsdtar; then
      tar --format ustar --uid 0 --gid 0 --uname '' --gname '' --no-mac-metadata \
        -cf "$out" -C "$data/art-cache" -T "$list"
    else
      tar --format ustar --owner=0 --group=0 --numeric-owner \
        -cf "$out" -C "$data/art-cache" -T "$list"
    fi
    (command -v sha256sum >/dev/null && sha256sum "$out" || shasum -a 256 "$out")
    echo "$(wc -l < "$list") files packed"
    ;;
  *)
    echo "usage: art-pack.sh fetch | make OUT.tar" >&2
    exit 2
    ;;
esac
