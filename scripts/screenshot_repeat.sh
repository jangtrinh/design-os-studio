#!/usr/bin/env bash
# Repeat screenshot captures and preserve actionable stderr for every failure.
# Usage: scripts/screenshot_repeat.sh [runs] [--mode idle|load|both] [--load] [--bin PATH]
# The default is one capture in idle mode so invoking this helper never opens a
# stream of short-lived GUI windows by accident. Pass an explicit count for stress runs.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

runs=1
mode="idle"
bin="${DESIGNOS_BIN:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      mode="${2:-}"
      shift 2
      ;;
    --load)
      mode="load"
      shift
      ;;
    --runs)
      runs="${2:-}"
      shift 2
      ;;
    --bin)
      bin="${2:-}"
      shift 2
      ;;
    [0-9]*)
      runs="$1"
      shift
      ;;
    *)
      echo "screenshot_repeat.sh: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$bin" ]]; then
  if [[ -n "${CARGO_TARGET_DIR:-}" && -x "$CARGO_TARGET_DIR/release/designos" ]]; then
    bin="$CARGO_TARGET_DIR/release/designos"
  elif [[ -n "${CARGO_TARGET_DIR:-}" && -x "$CARGO_TARGET_DIR/debug/designos" ]]; then
    bin="$CARGO_TARGET_DIR/debug/designos"
  else
    target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
    if [[ -x "$target_dir/release/designos" ]]; then
      bin="$target_dir/release/designos"
    else
      bin="$target_dir/debug/designos"
    fi
  fi
fi

if [[ ! "$runs" =~ ^[1-9][0-9]*$ ]]; then
  echo "screenshot_repeat.sh: runs must be a positive integer" >&2
  exit 2
fi
if [[ "$mode" != "idle" && "$mode" != "load" && "$mode" != "both" ]]; then
  echo "screenshot_repeat.sh: --mode must be idle, load, or both" >&2
  exit 2
fi
if [[ ! -x "$bin" ]]; then
  echo "screenshot_repeat.sh: binary not executable: $bin" >&2
  exit 2
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/designos-screenshot-repeat.XXXXXX")
load_pids=()
cleanup() {
  if (( ${#load_pids[@]} > 0 )); then
    for pid in "${load_pids[@]}"; do
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    done
    load_pids=()
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT INT TERM
mkdir -p "$tmp/home" "$tmp/tmp"

start_cpu_load() {
  local cores=4
  if command -v sysctl >/dev/null 2>&1; then
    cores=$(sysctl -n hw.ncpu 2>/dev/null || echo 4)
  elif command -v nproc >/dev/null 2>&1; then
    cores=$(nproc 2>/dev/null || echo 4)
  fi
  local workers=$((cores - 2))
  (( workers < 1 )) && workers=1
  load_pids=()
  for ((w = 1; w <= workers; w++)); do
    yes >/dev/null 2>&1 </dev/null &
    load_pids+=("$!")
  done
  echo "screenshot-repeat: started $workers CPU load workers (cores=$cores)"
}

stop_cpu_load() {
  if (( ${#load_pids[@]} > 0 )); then
    for pid in "${load_pids[@]}"; do
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    done
    load_pids=()
  fi
}

spec_json="$tmp/apply-spec.json"
spec_doc="$tmp/apply-spec.designos"
cat >"$spec_json" <<'EOF'
{
  "kind": "frame",
  "name": "ShotCheckCard",
  "x": 0,
  "y": 0,
  "width": 480,
  "height": 320,
  "fill": "#18181B",
  "corner_radius": 12,
  "layout": {"direction": "column", "gap": 16},
  "children": [
    {
      "kind": "frame",
      "name": "Header",
      "width": 432,
      "height": 48,
      "fill": "#27272A",
      "corner_radius": 8,
      "layout": {"direction": "row", "gap": 8},
      "children": [
        {
          "kind": "text",
          "name": "Title",
          "text": "Screenshot Capture Repeat",
          "font_size": 16,
          "fill": "#F4F4F6"
        }
      ]
    }
  ]
}
EOF

env HOME="$tmp/home" TMPDIR="$tmp/tmp/" GEMINI_CLI_HOME="$tmp/home" CODEX_HOME="$tmp/home/.codex" CLAUDE_CONFIG_DIR="$tmp/home/.claude" \
  "$bin" "$spec_doc" --apply-spec "$spec_json" >/dev/null

cases=(
  "cpu|$spec_doc --select none --size 1440x900"
  "gpu|$spec_doc --select none --size 1440x900 --gpu"
  "menu|$spec_doc --menu main --size 1440x900"
  "panel|$spec_doc --select Header --panel picker --size 1440x900"
  "apply-spec|$spec_doc --select none --size 1600x1000"
)

run_pass() {
  local pass_mode="$1"
  if [[ "$pass_mode" == "load" ]]; then
    start_cpu_load
  fi

  local total=0
  local failures=0
  local unexplained=0
  for spec in "${cases[@]}"; do
    local name=${spec%%|*}
    local args=${spec#*|}
    for ((i = 1; i <= runs; i++)); do
      total=$((total + 1))
      local png="$tmp/$pass_mode-$name-$i.png"
      local err="$tmp/$pass_mode-$name-$i.stderr"
      rm -f "$png" "$err"
      # shellcheck disable=SC2086
      if env HOME="$tmp/home" TMPDIR="$tmp/tmp/" GEMINI_CLI_HOME="$tmp/home" CODEX_HOME="$tmp/home/.codex" CLAUDE_CONFIG_DIR="$tmp/home/.claude" \
        "$bin" $args --screenshot "$png" >"$tmp/$pass_mode-$name-$i.stdout" 2>"$err"; then
        status=0
      else
        status=$?
      fi
      if (( status == 0 )) && [[ -s "$png" ]]; then
        rm -f "$png" "$err"
        continue
      fi
      failures=$((failures + 1))
      local png_state=missing
      [[ -s "$png" ]] && png_state=present
      if grep -Eq '^(designos: screenshot was not captured: |designos: cannot write )' "$err"; then
        local reason
        reason=$(grep -E '^(designos: screenshot was not captured: |designos: cannot write )' "$err" | tail -1)
        echo "FAIL [$pass_mode] $name/$i: exit=$status png=$png_state: $reason" >&2
      else
        unexplained=$((unexplained + 1))
        local reason
        if [[ -s "$err" ]]; then
          reason=$(tail -1 "$err")
        elif (( status != 0 )); then
          reason="process exited $status without a diagnostic"
        else
          reason="process exited 0 without writing a PNG"
        fi
        echo "FAIL [$pass_mode] $name/$i: exit=$status png=$png_state: $reason" >&2
        if [[ -s "$err" ]]; then cat "$err" >&2; fi
      fi
      rm -f "$png" "$err"
    done
  done

  if [[ "$pass_mode" == "load" ]]; then
    stop_cpu_load
  fi

  echo "screenshot-repeat ($pass_mode): $((total - failures))/$total captures passed (failures=$failures unexplained=$unexplained)"
  if (( failures > 0 )); then
    echo "screenshot-repeat ($pass_mode): $failures failures; see stderr lines above" >&2
    return 1
  fi
  return 0
}

if [[ "$mode" == "both" ]]; then
  run_pass idle
  run_pass load
else
  run_pass "$mode"
fi
