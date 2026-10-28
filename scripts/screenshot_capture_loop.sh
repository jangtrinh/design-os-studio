#!/usr/bin/env bash
# Stress loop for `--screenshot` capture reliability (#1141, US-17 Case 8).
#
# Runs N screenshot captures (default: 250) in `idle` mode and/or `load` mode
# (with a CPU load generator `yes > /dev/null` on `cores - 2` workers) against
# a small self-generated `.designos` fixture. Verifies that every run either
# writes a non-empty PNG and exits 0, or exits 1 with an actionable reason
# prefixed by `designos: screenshot was not captured: `.
#
# Usage:
#   scripts/screenshot_capture_loop.sh [--mode idle|load|both] [--runs 250] [--jobs 1] [--bin PATH]

set -euo pipefail

MODE="both"
RUNS=250
JOBS=1
BIN="${DESIGNOS_BIN:-}"

while [ $# -gt 0 ]; do
  case "$1" in
    --mode)
      MODE="$2"
      shift 2
      ;;
    --runs)
      RUNS="$2"
      shift 2
      ;;
    --jobs)
      JOBS="$2"
      shift 2
      ;;
    --bin)
      BIN="$2"
      shift 2
      ;;
    [0-9]*)
      RUNS="$1"
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [ -z "$BIN" ]; then
  if [ -n "${CARGO_TARGET_DIR:-}" ] && [ -x "$CARGO_TARGET_DIR/release/designos" ]; then
    BIN="$CARGO_TARGET_DIR/release/designos"
  elif [ -n "${CARGO_TARGET_DIR:-}" ] && [ -x "$CARGO_TARGET_DIR/debug/designos" ]; then
    BIN="$CARGO_TARGET_DIR/debug/designos"
  elif [ -x "$REPO_ROOT/out/cargo/release/designos" ]; then
    BIN="$REPO_ROOT/out/cargo/release/designos"
  elif [ -x "$REPO_ROOT/out/cargo/debug/designos" ]; then
    BIN="$REPO_ROOT/out/cargo/debug/designos"
  elif [ -x "$REPO_ROOT/target/release/designos" ]; then
    BIN="$REPO_ROOT/target/release/designos"
  elif [ -x "$REPO_ROOT/target/debug/designos" ]; then
    BIN="$REPO_ROOT/target/debug/designos"
  else
    echo "designos binary not found; set DESIGNOS_BIN or build designos first" >&2
    exit 2
  fi
fi

T0="${TMPDIR:-/tmp}"
T0="${T0%/}"
WORK_DIR="$(mktemp -d "$T0/designos-shot-loop.XXXXXX")"
mkdir -p "$WORK_DIR/home" "$WORK_DIR/out" "$WORK_DIR/res"

LOAD_PIDS=()
cleanup() {
  if [ "${#LOAD_PIDS[@]}" -gt 0 ]; then
    for pid in "${LOAD_PIDS[@]}"; do
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    done
    LOAD_PIDS=()
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT INT TERM

start_cpu_load() {
  local cores=4
  if command -v sysctl >/dev/null 2>&1; then
    cores="$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
  elif command -v nproc >/dev/null 2>&1; then
    cores="$(nproc 2>/dev/null || echo 4)"
  fi
  local workers=$((cores - 2))
  if [ "$workers" -lt 1 ]; then
    workers=1
  fi
  LOAD_PIDS=()
  for _ in $(seq 1 "$workers"); do
    yes >/dev/null 2>&1 </dev/null &
    LOAD_PIDS+=("$!")
  done
  echo "started_cpu_load_workers=$workers (cores=$cores)"
}

stop_cpu_load() {
  if [ "${#LOAD_PIDS[@]}" -gt 0 ]; then
    for pid in "${LOAD_PIDS[@]}"; do
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    done
    LOAD_PIDS=()
  fi
}

# Build a small self-contained fixture via `--apply-spec` (never touches external or client files).
SPEC_JSON="$WORK_DIR/fixture-spec.json"
FIXTURE_DOC="$WORK_DIR/fixture.designos"
cat >"$SPEC_JSON" <<'EOF'
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
          "text": "Screenshot Capture Fixture",
          "font_size": 16,
          "fill": "#F4F4F6"
        }
      ]
    },
    {
      "kind": "frame",
      "name": "Body",
      "width": 432,
      "height": 200,
      "fill": "#1E1E22",
      "corner_radius": 8,
      "layout": {"direction": "column", "gap": 12},
      "children": [
        {
          "kind": "rect",
          "name": "AccentBar",
          "width": 160,
          "height": 28,
          "fill": "#E36323",
          "corner_radius": 6
        }
      ]
    }
  ]
}
EOF

env HOME="$WORK_DIR/home" TMPDIR="$WORK_DIR/" \
  GEMINI_CLI_HOME="$WORK_DIR/home" CODEX_HOME="$WORK_DIR/home/.codex" CLAUDE_CONFIG_DIR="$WORK_DIR/home/.claude" \
  "$BIN" "$FIXTURE_DOC" --apply-spec "$SPEC_JSON" >/dev/null

run_one_capture() {
  local idx="$1"
  local run_home="$2"
  local png="$WORK_DIR/out/shot-$idx.png"
  local err_file="$WORK_DIR/out/shot-$idx.err"
  local res_file="$WORK_DIR/res/shot-$idx.res"
  rm -f "$png" "$err_file" "$res_file"

  local variant=$((idx % 5))
  local variant_name=""
  local cmd_args=()
  case "$variant" in
    0)
      variant_name="fixture-cpu"
      cmd_args=("$FIXTURE_DOC" --select none --size 1440x900 --no-toast --screenshot "$png")
      ;;
    1)
      variant_name="fixture-gpu"
      cmd_args=("$FIXTURE_DOC" --gpu --select none --size 1600x1000 --no-toast --screenshot "$png")
      ;;
    2)
      variant_name="fixture-menu-main"
      cmd_args=("$FIXTURE_DOC" --menu main --size 1440x900 --no-toast --screenshot "$png")
      ;;
    3)
      variant_name="fixture-panel-picker"
      cmd_args=("$FIXTURE_DOC" --panel picker --size 1440x900 --no-toast --screenshot "$png")
      ;;
    4)
      variant_name="showcase-cpu"
      cmd_args=(--fixture showcase --select none --size 1440x900 --no-toast --screenshot "$png")
      ;;
  esac

  set +e
  env HOME="$run_home" TMPDIR="$WORK_DIR/" \
    GEMINI_CLI_HOME="$run_home" CODEX_HOME="$run_home/.codex" CLAUDE_CONFIG_DIR="$run_home/.claude" \
    "$BIN" "${cmd_args[@]}" >/dev/null 2>"$err_file"
  local rc=$?
  set -e

  if [ "$rc" -eq 0 ] && [ -s "$png" ]; then
    echo "PASS|$idx|$variant_name|0|" >"$res_file"
  else
    local reason
    reason="$(tr '\n' ' ' <"$err_file" | sed 's/[[:space:]]*$//')"
    echo "FAIL|$idx|$variant_name|$rc|$reason" >"$res_file"
  fi
  rm -f "$png" "$err_file"
}

run_worker() {
  local worker_id="$1"
  local worker_home="$WORK_DIR/home/w-$worker_id"
  mkdir -p "$worker_home"
  local idx="$worker_id"
  while [ "$idx" -le "$RUNS" ]; do
    run_one_capture "$idx" "$worker_home"
    idx=$((idx + JOBS))
  done
}

run_suite() {
  local suite_mode="$1"
  echo "=== Case 8 suite: mode=$suite_mode runs=$RUNS jobs=$JOBS ==="
  rm -f "$WORK_DIR/res/"*.res
  if [ "$suite_mode" = "load" ]; then
    start_cpu_load
  fi

  if [ "$JOBS" -le 1 ]; then
    run_worker 1
  else
    local worker_pids=()
    for w in $(seq 1 "$JOBS"); do
      run_worker "$w" &
      worker_pids+=("$!")
    done
    for wpid in "${worker_pids[@]}"; do
      wait "$wpid"
    done
  fi

  if [ "$suite_mode" = "load" ]; then
    stop_cpu_load
  fi

  local passed=0
  local failed=0
  local unexplained=0
  for i in $(seq 1 "$RUNS"); do
    local res_file="$WORK_DIR/res/shot-$i.res"
    if [ ! -f "$res_file" ]; then
      failed=$((failed + 1))
      unexplained=$((unexplained + 1))
      echo "failure: run=$i/$RUNS mode=$suite_mode status=missing_result"
      continue
    fi
    IFS='|' read -r status idx vname rc reason <"$res_file"
    if [ "$status" = "PASS" ]; then
      passed=$((passed + 1))
    else
      failed=$((failed + 1))
      echo "failure: run=$idx/$RUNS mode=$suite_mode variant=$vname exit=$rc stderr=$reason"
      case "$reason" in
        *"designos: screenshot was not captured: "?*) ;;
        *)
          unexplained=$((unexplained + 1))
          ;;
      esac
    fi
  done

  echo "summary: mode=$suite_mode total=$RUNS passed=$passed failed=$failed unexplained=$unexplained"
  if [ "$unexplained" -gt 0 ]; then
    return 1
  fi
  return 0
}

case "$MODE" in
  idle)
    run_suite idle
    ;;
  load)
    run_suite load
    ;;
  both)
    run_suite idle
    run_suite load
    ;;
  *)
    echo "Unsupported --mode: $MODE (expected idle, load, or both)" >&2
    exit 2
    ;;
esac
