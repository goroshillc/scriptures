#!/usr/bin/env bash
# Every check CI runs: Rust format, lints and tests, the browser glue's own formatter and
# linter (Deno), and the scripts' linter (ShellCheck).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --release -- -D warnings
cargo test --workspace --release
(cd crates/scriptures && deno fmt --check && deno lint)
shellcheck scripts/*.sh
echo "all checks passed"
