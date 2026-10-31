#!/usr/bin/env bash
# Repeat screenshot captures and preserve actionable stderr for every failure.
# Usage: scripts/screenshot_repeat.sh [runs]
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
runs=${1:-250}
if [[ -n "${DESIGNOS_BIN:-}" ]]; then
  bin=$DESIGNOS_BIN
else
  target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
  bin="$target_dir/debug/designos"
fi

if [[ ! "$runs" =~ ^[1-9][0-9]*$ ]]; then
  echo "screenshot_repeat.sh: runs must be a positive integer" >&2
  exit 2
fi
if [[ ! -x "$bin" ]]; then
  echo "screenshot_repeat.sh: binary not executable: $bin" >&2
  exit 2
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/designos-screenshot-repeat.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

cases=(
  "cpu|--fixture showcase --select none --size 1440x900"
  "gpu|--fixture showcase --select none --size 1440x900 --gpu"
  "menu|--fixture showcase --menu main --size 1440x900"
  "panel|--fixture showcase --panel picker --size 1440x900"
)

total=0
failures=0
for spec in "${cases[@]}"; do
  name=${spec%%|*}
  args=${spec#*|}
  for ((i = 1; i <= runs; i++)); do
    total=$((total + 1))
    png="$tmp/$name-$i.png"
    err="$tmp/$name-$i.stderr"
    # shellcheck disable=SC2086
    if "$bin" $args --screenshot "$png" >"$tmp/$name-$i.stdout" 2>"$err"; then
      status=0
    else
      status=$?
    fi
    if (( status == 0 )) && [[ -s "$png" ]]; then
      continue
    fi
    failures=$((failures + 1))
    png_state=missing
    [[ -s "$png" ]] && png_state=present
    if grep -Eq '^(designos: screenshot was not captured: |designos: cannot write )' "$err"; then
      reason=$(grep -E '^(designos: screenshot was not captured: |designos: cannot write )' "$err" | tail -1)
      echo "FAIL $name/$i: exit=$status png=$png_state: $reason" >&2
    else
      if [[ -s "$err" ]]; then
        reason=$(tail -1 "$err")
      elif (( status != 0 )); then
        reason="process exited $status without a diagnostic"
      else
        reason="process exited 0 without writing a PNG"
      fi
      echo "FAIL $name/$i: exit=$status png=$png_state: $reason" >&2
      if [[ -s "$err" ]]; then cat "$err" >&2; fi
    fi
  done
done

echo "screenshot-repeat: $((total - failures))/$total captures passed"
if (( failures > 0 )); then
  echo "screenshot-repeat: $failures failures; see stderr lines above" >&2
  exit 1
fi
