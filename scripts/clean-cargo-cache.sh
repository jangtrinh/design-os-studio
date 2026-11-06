#!/usr/bin/env bash
# Remove only the shared Cargo target cache after confirming no Cargo compiler is running.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
if pgrep -f '(^|/)(cargo|rustc)( |$)' >/dev/null 2>&1; then
  echo "clean-cargo-cache.sh: cargo/rustc is running; refusing to remove the cache" >&2
  exit 1
fi

if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  target_dir="$CARGO_TARGET_DIR"
else
  target_dir="$(python3 "$root/scripts/cargo_target.py" "$root")"
fi
target_dir="$(python3 "$root/scripts/cargo_target.py" --check-clean "$root" "$target_dir")"

rm -rf "$target_dir"
echo "Removed $target_dir"
