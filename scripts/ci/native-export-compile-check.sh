#!/usr/bin/env bash
# Compile-check native code exports (SwiftUI, UIKit, Jetpack Compose, Android XML).
#
# Usage:
#   ./scripts/ci/native-export-compile-check.sh [--target <all|swift|kotlin|android>]... [--fixture-dir <dir>] [--self-test]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

TARGETS=()
EXPLICIT_TARGET=0
FIXTURE_DIR=""
SELF_TEST=0

usage() {
  cat <<'EOF'
Usage: native-export-compile-check.sh [OPTIONS]

Options:
  --target <all|swift|kotlin|android>  Target(s) to check (repeatable; default: all)
  --fixture-dir <dir>                  Directory of pre-generated fixtures to check
  --self-test                          Verify planted-error fixtures fail, then verify real fixtures pass
  -h, --help                           Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      [[ $# -ge 2 ]] || { echo "ERROR: --target requires a value" >&2; exit 2; }
      TARGETS+=("$2")
      EXPLICIT_TARGET=1
      shift 2
      ;;
    --fixture-dir)
      [[ $# -ge 2 ]] || { echo "ERROR: --fixture-dir requires a value" >&2; exit 2; }
      FIXTURE_DIR="$2"
      shift 2
      ;;
    --self-test)
      SELF_TEST=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  TARGETS=("all")
fi

RUN_SWIFT=0
RUN_KOTLIN=0
RUN_ANDROID=0
for t in "${TARGETS[@]}"; do
  case "$t" in
    all)
      RUN_SWIFT=1
      RUN_KOTLIN=1
      RUN_ANDROID=1
      ;;
    swift)
      RUN_SWIFT=1
      ;;
    kotlin)
      RUN_KOTLIN=1
      ;;
    android)
      RUN_ANDROID=1
      ;;
    *)
      echo "ERROR: unknown --target '$t' (expected all, swift, kotlin, or android)" >&2
      exit 2
      ;;
  esac
done

ORIG_HOME="${HOME:-/tmp}"
export RUSTUP_HOME="${RUSTUP_HOME:-$ORIG_HOME/.rustup}"
export CARGO_HOME="${CARGO_HOME:-$ORIG_HOME/.cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

SCRATCH_ROOT="$(cd "$REPO_ROOT/.." && pwd)/.test-scratch/native-compile-$$"
mkdir -p "$SCRATCH_ROOT/home" "$SCRATCH_ROOT/tmp"
cleanup() {
  rm -rf "$SCRATCH_ROOT"
}
trap cleanup EXIT

dump_real_fixtures() {
  local out_dir="$1"
  mkdir -p "$out_dir" "$REPO_ROOT/out/cargo"
  echo "==> Dumping native export fixtures via designos-export test binary..."
  NATIVE_COMPILE_DUMP_DIR="$out_dir" \
    HOME="$SCRATCH_ROOT/home" \
    TMPDIR="$SCRATCH_ROOT/tmp/" \
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/out/cargo}" \
    cargo test -p designos-export --test native_compile_fixtures --locked dump_native_compile_fixtures -- --exact --nocapture
}

check_swift_dir() {
  local dir="$1"
  if ! command -v swiftc >/dev/null 2>&1 || ! command -v xcrun >/dev/null 2>&1; then
    if [[ $EXPLICIT_TARGET -eq 1 ]]; then
      echo "ERROR: swiftc/xcrun not found for --target swift" >&2
      return 1
    fi
    echo "SKIP: swift (swiftc/xcrun not found)"
    return 0
  fi

  local arch
  arch="$(uname -m)"
  local macos_target="${arch}-apple-macosx14.0"
  local ios_sim_target="${arch}-apple-ios17.0-simulator"
  local status=0
  local count=0

  shopt -s nullglob
  local swiftui_files=("$dir/swiftui"/*.swift)
  local uikit_files=("$dir/uikit"/*.swift)
  shopt -u nullglob

  if [[ ${#swiftui_files[@]} -eq 0 && ${#uikit_files[@]} -eq 0 ]]; then
    echo "ERROR: no Swift fixtures found under $dir/{swiftui,uikit}" >&2
    return 1
  fi

  for f in "${swiftui_files[@]}"; do
    count=$((count + 1))
    local rel="swiftui/$(basename "$f")"
    if xcrun --sdk macosx swiftc -typecheck -target "$macos_target" "$f"; then
      echo "[PASS] $rel (swiftc -typecheck -sdk macosx)"
    else
      echo "[FAIL] $rel (swiftc -typecheck -sdk macosx)" >&2
      status=1
    fi
  done

  for f in "${uikit_files[@]}"; do
    count=$((count + 1))
    local rel="uikit/$(basename "$f")"
    if xcrun --sdk iphonesimulator swiftc -typecheck -target "$ios_sim_target" "$f"; then
      echo "[PASS] $rel (swiftc -typecheck -sdk iphonesimulator)"
    else
      echo "[FAIL] $rel (swiftc -typecheck -sdk iphonesimulator)" >&2
      status=1
    fi
  done

  echo "==> Swift check finished ($count files, exit=$status)"
  return $status
}

check_kotlin_dir() {
  local dir="$1"
  if ! command -v kotlinc >/dev/null 2>&1; then
    if [[ $EXPLICIT_TARGET -eq 1 ]]; then
      echo "ERROR: kotlinc not found for --target kotlin" >&2
      return 1
    fi
    echo "SKIP: kotlin (kotlinc not found)"
    return 0
  fi

  local stubs="$REPO_ROOT/scripts/ci/compose-stubs.kt"
  if [[ ! -f "$stubs" ]]; then
    echo "ERROR: missing Compose stubs file at $stubs" >&2
    return 1
  fi

  shopt -s nullglob
  local kt_files=("$dir/compose"/*.kt)
  shopt -u nullglob

  if [[ ${#kt_files[@]} -eq 0 ]]; then
    echo "ERROR: no Kotlin Compose fixtures found under $dir/compose" >&2
    return 1
  fi

  local out_jar="$SCRATCH_ROOT/tmp/compose-fixtures.jar"
  rm -f "$out_jar"
  echo "==> Compiling ${#kt_files[@]} Compose fixture(s) with kotlinc..."
  if kotlinc "$stubs" "${kt_files[@]}" -d "$out_jar"; then
    for f in "${kt_files[@]}"; do
      echo "[PASS] compose/$(basename "$f") (kotlinc)"
    done
    echo "==> Kotlin Compose check finished (${#kt_files[@]} files, exit=0)"
    return 0
  else
    echo "[FAIL] compose/*.kt (kotlinc compilation failed)" >&2
    return 1
  fi
}

check_android_dir() {
  local dir="$1"
  if ! command -v aapt2 >/dev/null 2>&1; then
    if [[ $EXPLICIT_TARGET -eq 1 ]]; then
      echo "ERROR: aapt2 not found for --target android" >&2
      return 1
    fi
    echo "SKIP: android (aapt2 not found)"
    return 0
  fi

  local xml_files=()
  while IFS= read -r -d '' f; do
    xml_files+=("$f")
  done < <(find "$dir/android-xml" -type f -name '*.xml' -print0 2>/dev/null | sort -z)

  if [[ ${#xml_files[@]} -eq 0 ]]; then
    echo "ERROR: no Android XML fixtures found under $dir/android-xml" >&2
    return 1
  fi

  local out_dir="$SCRATCH_ROOT/tmp/aapt2-out"
  mkdir -p "$out_dir"
  local status=0
  local count=0

  for f in "${xml_files[@]}"; do
    count=$((count + 1))
    local rel="${f#"$dir/"}"
    if aapt2 compile "$f" -o "$out_dir"; then
      echo "[PASS] $rel (aapt2 compile)"
    else
      echo "[FAIL] $rel (aapt2 compile)" >&2
      status=1
    fi
  done

  echo "==> Android XML check finished ($count files, exit=$status)"
  return $status
}

run_checks_on_dir() {
  local dir="$1"
  local overall=0

  if [[ $RUN_SWIFT -eq 1 ]]; then
    if ! check_swift_dir "$dir"; then
      overall=1
    fi
  fi

  if [[ $RUN_KOTLIN -eq 1 ]]; then
    if ! check_kotlin_dir "$dir"; then
      overall=1
    fi
  fi

  if [[ $RUN_ANDROID -eq 1 ]]; then
    if ! check_android_dir "$dir"; then
      overall=1
    fi
  fi

  return $overall
}

if [[ $SELF_TEST -eq 1 ]]; then
  PLANTED_DIR="$REPO_ROOT/scripts/ci/fixtures/planted-error"
  echo "================================================================"
  echo "STEP 1/2: Planted-error self-test (expecting failure)"
  echo "Fixture dir: $PLANTED_DIR"
  echo "================================================================"
  planted_failed=0
  if [[ $RUN_SWIFT -eq 1 ]] && command -v swiftc >/dev/null 2>&1 && command -v xcrun >/dev/null 2>&1; then
    if check_swift_dir "$PLANTED_DIR"; then
      echo "ERROR: planted-error Swift fixture unexpectedly passed!" >&2
      exit 1
    else
      echo "[PASS] Planted-error Swift fixture rejected as expected."
      planted_failed=1
    fi
  fi
  if [[ $RUN_KOTLIN -eq 1 ]] && command -v kotlinc >/dev/null 2>&1; then
    if check_kotlin_dir "$PLANTED_DIR"; then
      echo "ERROR: planted-error Kotlin fixture unexpectedly passed!" >&2
      exit 1
    else
      echo "[PASS] Planted-error Kotlin fixture rejected as expected."
      planted_failed=1
    fi
  fi
  if [[ $RUN_ANDROID -eq 1 ]] && command -v aapt2 >/dev/null 2>&1; then
    if check_android_dir "$PLANTED_DIR"; then
      echo "ERROR: planted-error Android XML fixture unexpectedly passed!" >&2
      exit 1
    else
      echo "[PASS] Planted-error Android XML fixture rejected as expected."
      planted_failed=1
    fi
  fi
  if [[ $planted_failed -eq 0 ]]; then
    echo "ERROR: no compiler toolchain available to run planted-error self-test" >&2
    exit 1
  fi

  echo "================================================================"
  echo "STEP 2/2: Real exporter fixtures compile check (expecting pass)"
  echo "================================================================"
fi

if [[ -n "$FIXTURE_DIR" ]]; then
  TARGET_DIR="$FIXTURE_DIR"
else
  TARGET_DIR="$SCRATCH_ROOT/tmp/real-fixtures"
  dump_real_fixtures "$TARGET_DIR"
fi

run_checks_on_dir "$TARGET_DIR"
