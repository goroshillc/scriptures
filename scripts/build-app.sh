#!/usr/bin/env bash
# Builds the desktop app: the desktop edition of the reader inside a Tauri 2 bundle.
#
#   scripts/build-app.sh                    # every installer this system can make
#   scripts/build-app.sh --bundles app      # extra arguments go to `tauri build`
#
# TAURI overrides the CLI (default `cargo tauri`; CI uses the pinned npm build).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
site="$root/crates/scriptures-app/site"
if [[ -e "$site" ]]; then
  # Only a previous build output (it carries the builder's marker) is ever replaced.
  [[ -f "$site/.goroshi-scriptures-dist" ]] || { echo "$site is not a build output; refusing" >&2; exit 2; }
  rm -rf "$site"
fi
"$root/scripts/build-site.sh" desktop "$site"
cd "$root/crates/scriptures-app"
${TAURI:-cargo tauri} build --config tauri.bundle.json "$@"
