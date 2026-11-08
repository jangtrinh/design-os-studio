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
  --self-test                          Verify every planted-error fixture fails with its
                                       expected diagnostic, then verify real fixtures pass
  -h, --help                           Show this help message

Environment:
  CARGO_TARGET_DIR                     Cargo build output directory (default: <repo>/target)
  COMPOSE_COMPILER_PLUGIN_JAR          Path to kotlin-compose-compiler-plugin.jar / compose-compiler-plugin.jar
  ANDROID_JAR                          Path to Android platform android.jar for `aapt2 link`
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

SCRATCH_BASE="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
SCRATCH_ROOT="$(mktemp -d "${SCRATCH_BASE%/}/designos-native-compile.XXXXXX")"
mkdir -p "$SCRATCH_ROOT/home" "$SCRATCH_ROOT/tmp"
cleanup() {
  rm -rf "$SCRATCH_ROOT"
}
trap cleanup EXIT

dump_real_fixtures() {
  local out_dir="$1"
  local cargo_target="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
  mkdir -p "$out_dir" "$cargo_target"
  echo "==> Dumping native export fixtures via designos-export test binary..."
  NATIVE_COMPILE_DUMP_DIR="$out_dir" \
    HOME="$SCRATCH_ROOT/home" \
    TMPDIR="$SCRATCH_ROOT/tmp/" \
    CARGO_TARGET_DIR="$cargo_target" \
    cargo test -p designos-export --test native_compile_fixtures --locked dump_native_compile_fixtures -- --exact --nocapture
}

resolve_compose_plugin_jar() {
  if [[ -n "${COMPOSE_COMPILER_PLUGIN_JAR:-}" && -f "${COMPOSE_COMPILER_PLUGIN_JAR}" ]]; then
    echo "${COMPOSE_COMPILER_PLUGIN_JAR}"
    return 0
  fi
  local kotlinc_bin
  kotlinc_bin="$(command -v kotlinc || true)"
  if [[ -z "$kotlinc_bin" ]]; then
    return 1
  fi
  local resolved_bin="$kotlinc_bin"
  if command -v realpath >/dev/null 2>&1; then
    resolved_bin="$(realpath "$kotlinc_bin" 2>/dev/null || echo "$kotlinc_bin")"
  fi
  local lib_dir
  lib_dir="$(cd "$(dirname "$resolved_bin")/.." && pwd)/lib"
  for candidate in \
    "$lib_dir/kotlin-compose-compiler-plugin.jar" \
    "$lib_dir/compose-compiler-plugin.jar" \
    "/opt/kotlinc/lib/kotlin-compose-compiler-plugin.jar" \
    "/opt/kotlinc/lib/compose-compiler-plugin.jar"; do
    if [[ -f "$candidate" ]]; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}

resolve_android_jar() {
  if [[ -n "${ANDROID_JAR:-}" && -f "${ANDROID_JAR}" ]]; then
    echo "${ANDROID_JAR}"
    return 0
  fi
  local sdk_root="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/usr/local/lib/android/sdk}}"
  if [[ -f "$sdk_root/platforms/android-34/android.jar" ]]; then
    echo "$sdk_root/platforms/android-34/android.jar"
    return 0
  fi
  local found
  found="$(find "$sdk_root/platforms" -maxdepth 2 -name android.jar 2>/dev/null | sort -V | tail -n 1 || true)"
  if [[ -n "$found" && -f "$found" ]]; then
    echo "$found"
    return 0
  fi
  return 1
}

ensure_compose_stubs_jar() {
  local plugin_jar="$1"
  local stubs_jar="$SCRATCH_ROOT/tmp/compose-stubs.jar"
  if [[ -f "$stubs_jar" ]]; then
    return 0
  fi
  echo "==> Pre-compiling Compose runtime + UI stubs into $(basename "$stubs_jar")..."
  kotlinc \
    "-Xplugin=$plugin_jar" \
    "$REPO_ROOT/scripts/ci/compose-runtime-stubs.kt" \
    "$REPO_ROOT/scripts/ci/compose-runtime-internal-stubs.kt" \
    "$REPO_ROOT/scripts/ci/compose-stubs.kt" \
    -d "$stubs_jar"
}

run_kotlinc_compose() {
  local plugin_jar="$1"
  local out_jar="$2"
  shift 2
  local stubs_jar="$SCRATCH_ROOT/tmp/compose-stubs.jar"
  kotlinc \
    "-Xplugin=$plugin_jar" \
    -cp "$stubs_jar" \
    "$@" \
    -d "$out_jar"
}

compile_android_stubs() {
  local stub_flat_dir="$SCRATCH_ROOT/tmp/aapt2-stubs-flat"
  mkdir -p "$stub_flat_dir"
  local stub_res_dir="$REPO_ROOT/scripts/ci/android-stubs/res"
  local stub_files=()
  while IFS= read -r -d '' sf; do
    stub_files+=("$sf")
  done < <(find "$stub_res_dir" -type f -name '*.xml' -print0 | sort -z)
  for sf in "${stub_files[@]}"; do
    aapt2 compile "$sf" -o "$stub_flat_dir"
  done
  echo "$stub_flat_dir"
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

  local plugin_jar
  if ! plugin_jar="$(resolve_compose_plugin_jar)"; then
    echo "ERROR: Compose compiler plugin jar (kotlin-compose-compiler-plugin.jar / compose-compiler-plugin.jar) not found" >&2
    return 1
  fi
  ensure_compose_stubs_jar "$plugin_jar"

  shopt -s nullglob
  local kt_files=("$dir/compose"/*.kt)
  shopt -u nullglob

  if [[ ${#kt_files[@]} -eq 0 ]]; then
    echo "ERROR: no Kotlin Compose fixtures found under $dir/compose" >&2
    return 1
  fi

  local out_jar="$SCRATCH_ROOT/tmp/compose-fixtures.jar"
  rm -f "$out_jar"
  echo "==> Compiling ${#kt_files[@]} Compose fixture(s) with kotlinc + $(basename "$plugin_jar")..."
  if run_kotlinc_compose "$plugin_jar" "$out_jar" "${kt_files[@]}"; then
    for f in "${kt_files[@]}"; do
      echo "[PASS] compose/$(basename "$f") (kotlinc -Xplugin=$(basename "$plugin_jar"))"
    done
    echo "==> Kotlin Compose check finished (${#kt_files[@]} files, exit=0)"
    return 0
  fi

  local status=1
  for f in "${kt_files[@]}"; do
    local single_jar="$SCRATCH_ROOT/tmp/compose-single.jar"
    rm -f "$single_jar"
    if run_kotlinc_compose "$plugin_jar" "$single_jar" "$f"; then
      echo "[PASS] compose/$(basename "$f") (kotlinc -Xplugin=$(basename "$plugin_jar"))"
    else
      echo "[FAIL] compose/$(basename "$f") (kotlinc -Xplugin=$(basename "$plugin_jar"))" >&2
    fi
  done
  echo "==> Kotlin Compose check finished (${#kt_files[@]} files, exit=$status)" >&2
  return $status
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

  local android_jar
  if ! android_jar="$(resolve_android_jar)"; then
    echo "ERROR: android.jar not found for aapt2 link (set ANDROID_JAR or install platforms;android-34)" >&2
    return 1
  fi

  shopt -s nullglob
  local scene_dirs=("$dir/android-xml"/*)
  shopt -u nullglob

  if [[ ${#scene_dirs[@]} -eq 0 ]]; then
    echo "ERROR: no Android XML scene directories found under $dir/android-xml" >&2
    return 1
  fi

  local stub_flat_dir
  stub_flat_dir="$(compile_android_stubs)"
  shopt -s nullglob
  local stub_flats=("$stub_flat_dir"/*.flat)
  shopt -u nullglob

  local manifest="$REPO_ROOT/scripts/ci/android-stubs/AndroidManifest.xml"
  local status=0
  local file_count=0
  local scene_count=0

  for scene_dir in "${scene_dirs[@]}"; do
    [[ -d "$scene_dir" ]] || continue
    scene_count=$((scene_count + 1))
    local scene_name
    scene_name="$(basename "$scene_dir")"
    local scene_flat_dir="$SCRATCH_ROOT/tmp/aapt2-scene-$scene_name"
    rm -rf "$scene_flat_dir"
    mkdir -p "$scene_flat_dir"

    local xml_files=()
    while IFS= read -r -d '' xf; do
      xml_files+=("$xf")
    done < <(find "$scene_dir" -type f -name '*.xml' -print0 2>/dev/null | sort -z)

    if [[ ${#xml_files[@]} -eq 0 ]]; then
      echo "[FAIL] android-xml/$scene_name (no XML files found)" >&2
      status=1
      continue
    fi

    local scene_compile_ok=1
    for xf in "${xml_files[@]}"; do
      file_count=$((file_count + 1))
      local rel="${xf#"$dir/"}"
      if aapt2 compile "$xf" -o "$scene_flat_dir"; then
        echo "[PASS] $rel (aapt2 compile)"
      else
        echo "[FAIL] $rel (aapt2 compile)" >&2
        scene_compile_ok=0
        status=1
      fi
    done

    if [[ $scene_compile_ok -eq 1 ]]; then
      shopt -s nullglob
      local scene_flats=("$scene_flat_dir"/*.flat)
      shopt -u nullglob
      local out_apk="$SCRATCH_ROOT/tmp/aapt2-link-$scene_name.apk"
      rm -f "$out_apk"
      if aapt2 link --auto-add-overlay -I "$android_jar" --manifest "$manifest" -o "$out_apk" "${stub_flats[@]}" "${scene_flats[@]}"; then
        echo "[PASS] android-xml/$scene_name (aapt2 link against $(basename "$(dirname "$android_jar")")/android.jar)"
      else
        echo "[FAIL] android-xml/$scene_name (aapt2 link against $(basename "$(dirname "$android_jar")")/android.jar)" >&2
        status=1
      fi
    fi
  done

  echo "==> Android XML check finished ($scene_count scenes, $file_count XML files, exit=$status)"
  return $status
}

expect_cmd_failure_with_pattern() {
  local label="$1"
  local expected_pattern="$2"
  shift 2

  local out_file="$SCRATCH_ROOT/tmp/planted-check.out"
  rm -f "$out_file"

  set +e
  "$@" >"$out_file" 2>&1
  local rc=$?
  set -e

  cat "$out_file"
  if [[ $rc -eq 0 ]]; then
    echo "[FAIL] Planted-error fixture $label unexpectedly succeeded (exit 0)!" >&2
    return 1
  fi
  if ! grep -Fq "$expected_pattern" "$out_file"; then
    echo "[FAIL] Planted-error fixture $label failed (exit $rc) but did not emit expected diagnostic '$expected_pattern'!" >&2
    return 1
  fi
  echo "[PASS] Planted-error fixture $label rejected as expected (matched '$expected_pattern')."
  return 0
}

run_planted_self_test() {
  local planted_dir="$REPO_ROOT/scripts/ci/fixtures/planted-error"
  local ran_any=0

  echo "================================================================"
  echo "STEP 1/2: Planted-error self-test (each planted file must fail with expected diagnostic)"
  echo "Fixture dir: $planted_dir"
  echo "================================================================"

  if [[ $RUN_SWIFT -eq 1 ]]; then
    if command -v swiftc >/dev/null 2>&1 && command -v xcrun >/dev/null 2>&1; then
      ran_any=1
      local arch
      arch="$(uname -m)"
      expect_cmd_failure_with_pattern \
        "swiftui/planted_broken_swiftui.swift" \
        "error: value of type 'some View' has no member 'nonExistentDesignOsModifier'" \
        xcrun --sdk macosx swiftc -typecheck -target "${arch}-apple-macosx14.0" \
        "$planted_dir/swiftui/planted_broken_swiftui.swift"

      expect_cmd_failure_with_pattern \
        "uikit/planted_broken_uikit.swift" \
        "error: type 'NSLayoutConstraint.Axis' has no member 'nonExistentAxisDirection'" \
        xcrun --sdk iphonesimulator swiftc -typecheck -target "${arch}-apple-ios17.0-simulator" \
        "$planted_dir/uikit/planted_broken_uikit.swift"
    elif [[ $EXPLICIT_TARGET -eq 1 ]]; then
      echo "ERROR: swiftc/xcrun not found for --target swift" >&2
      return 1
    fi
  fi

  if [[ $RUN_KOTLIN -eq 1 ]]; then
    if command -v kotlinc >/dev/null 2>&1; then
      ran_any=1
      local plugin_jar
      plugin_jar="$(resolve_compose_plugin_jar)"
      ensure_compose_stubs_jar "$plugin_jar"
      local out_jar="$SCRATCH_ROOT/tmp/planted-compose.jar"

      expect_cmd_failure_with_pattern \
        "compose/planted_broken_compose.kt" \
        "error: unresolved reference 'nonExistentModifierCall'." \
        run_kotlinc_compose "$plugin_jar" "$out_jar" \
        "$planted_dir/compose/planted_broken_compose.kt"

      expect_cmd_failure_with_pattern \
        "compose/planted_composable_in_grid_lambda.kt" \
        "error: @Composable invocations can only happen from the context of a @Composable function" \
        run_kotlinc_compose "$plugin_jar" "$out_jar" \
        "$planted_dir/compose/planted_composable_in_grid_lambda.kt"

      expect_cmd_failure_with_pattern \
        "compose/planted_weight_in_box_scope.kt" \
        "error: unresolved reference 'weight'." \
        run_kotlinc_compose "$plugin_jar" "$out_jar" \
        "$planted_dir/compose/planted_weight_in_box_scope.kt"

      expect_cmd_failure_with_pattern \
        "compose/planted_color_scheme_in_non_composable_lambda.kt" \
        "error: @Composable invocations can only happen from the context of a @Composable function" \
        run_kotlinc_compose "$plugin_jar" "$out_jar" \
        "$planted_dir/compose/planted_color_scheme_in_non_composable_lambda.kt"
    elif [[ $EXPLICIT_TARGET -eq 1 ]]; then
      echo "ERROR: kotlinc not found for --target kotlin" >&2
      return 1
    fi
  fi

  if [[ $RUN_ANDROID -eq 1 ]]; then
    if command -v aapt2 >/dev/null 2>&1; then
      ran_any=1
      local android_jar
      android_jar="$(resolve_android_jar)"
      local stub_flat_dir
      stub_flat_dir="$(compile_android_stubs)"
      shopt -s nullglob
      local stub_flats=("$stub_flat_dir"/*.flat)
      shopt -u nullglob
      local manifest="$REPO_ROOT/scripts/ci/android-stubs/AndroidManifest.xml"

      local planted_compile_dir="$SCRATCH_ROOT/tmp/planted-aapt2-01"
      mkdir -p "$planted_compile_dir"
      expect_cmd_failure_with_pattern \
        "android-xml/01_planted/res/layout/planted_broken.xml" \
        "error: not well-formed (invalid token)." \
        aapt2 compile "$planted_dir/android-xml/01_planted/res/layout/planted_broken.xml" -o "$planted_compile_dir"

      local planted_attr_dir="$SCRATCH_ROOT/tmp/planted-aapt2-02"
      mkdir -p "$planted_attr_dir"
      aapt2 compile "$planted_dir/android-xml/02_planted_bad_attr/res/layout/planted_bad_attr.xml" -o "$planted_attr_dir"
      echo "[PASS] android-xml/02_planted_bad_attr/res/layout/planted_bad_attr.xml (aapt2 compile succeeded prior to link check)"
      shopt -s nullglob
      local attr_flats=("$planted_attr_dir"/*.flat)
      shopt -u nullglob
      expect_cmd_failure_with_pattern \
        "android-xml/02_planted_bad_attr/res/layout/planted_bad_attr.xml (aapt2 link)" \
        "error: attribute android:layout_widht not found." \
        aapt2 link --auto-add-overlay -I "$android_jar" --manifest "$manifest" \
        -o "$SCRATCH_ROOT/tmp/planted-attr.apk" "${stub_flats[@]}" "${attr_flats[@]}"
    elif [[ $EXPLICIT_TARGET -eq 1 ]]; then
      echo "ERROR: aapt2 not found for --target android" >&2
      return 1
    fi
  fi

  if [[ $ran_any -eq 0 ]]; then
    echo "ERROR: no compiler toolchain available to run planted-error self-test" >&2
    return 1
  fi

  echo "================================================================"
  echo "STEP 2/2: Real exporter fixtures compile check (expecting pass)"
  echo "================================================================"
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
  run_planted_self_test
fi

if [[ -n "$FIXTURE_DIR" ]]; then
  TARGET_DIR="$FIXTURE_DIR"
else
  TARGET_DIR="$SCRATCH_ROOT/tmp/real-fixtures"
  dump_real_fixtures "$TARGET_DIR"
fi

run_checks_on_dir "$TARGET_DIR"
