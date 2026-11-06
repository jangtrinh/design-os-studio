#!/usr/bin/env bash
# Unified Local & CI Release Builder for design:os studio.
#
# Usage:
#   scripts/build-release.sh [version] [--local-host | --universal-macos | --generate-manifest | --skip-build]
#
# Examples:
#   scripts/build-release.sh 0.2.0 --local-host
#   scripts/build-release.sh 0.2.0 --universal-macos
#   scripts/build-release.sh 0.2.0 --generate-manifest
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  CARGO_TARGET_DIR="$(python3 "$root/scripts/cargo_target.py" "$root")"
fi
export CARGO_TARGET_DIR
target_dir="$CARGO_TARGET_DIR"

version=""
mode="local-host"
skip_build=0

for arg in "$@"; do
  case "$arg" in
    --local-host) mode="local-host" ;;
    --universal-macos) mode="universal-macos" ;;
    --generate-manifest) mode="generate-manifest" ;;
    --skip-build) skip_build=1 ;;
    --help|-h)
      echo "Usage: scripts/build-release.sh [version] [--local-host | --universal-macos | --generate-manifest | --skip-build]"
      exit 0
      ;;
    -*)
      echo "build-release.sh: unknown option: $arg" >&2
      exit 2
      ;;
    *)
      if [ -z "$version" ]; then
        version="${arg#v}"
      fi
      ;;
  esac
done

if [ -z "$version" ]; then
  version=$(awk -F'"' '/^version *= *"/ { print $2; exit }' "$root/Cargo.toml")
  version=${version:-0.2.0}
fi

mkdir -p "$root/dist"

generate_manifest_and_sums() {
  local ver=$1
  local dist_dir="$root/dist"
  cd "$dist_dir"

  # 1. Build SHA256SUMS from all .sha256 files or compute directly for release artifacts
  rm -f SHA256SUMS
  if ls *.sha256 >/dev/null 2>&1; then
    cat *.sha256 | sort -k2 -u > SHA256SUMS
  else
    for f in *.dmg *.tar.gz *.zip *.exe *.deb; do
      [ -f "$f" ] || continue
      if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$f" >> SHA256SUMS
      else
        shasum -a 256 "$f" >> SHA256SUMS
      fi
    done
  fi

  # Determine update channel from version string
  local channel="stable"
  case "$ver" in
    *beta*) channel="beta" ;;
    *nightly*|*dev*) channel="nightly" ;;
  esac

  local published_at
  published_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  local base_url="https://github.com/jangtrinh/design-os-studio/releases/download/v${ver}"
  local primary_file=""
  for candidate in \
    "designos-${ver}-macos-universal.tar.gz" \
    "designos-${ver}-macos-arm64.tar.gz" \
    "designos-${ver}-macos.dmg" \
    "designos-${ver}-x86_64-pc-windows-msvc-setup.exe" \
    "designos-${ver}-x86_64-pc-windows-msvc.zip" \
    "designos_${ver}_amd64.deb" \
    "designos-${ver}-x86_64-unknown-linux-gnu.tar.gz"; do
    if [ -f "$candidate" ]; then
      primary_file="$candidate"
      break
    fi
  done

  local primary_sha256=""
  if [ -n "$primary_file" ] && [ -f SHA256SUMS ]; then
    primary_sha256=$(awk -v f="$primary_file" '$2 == f || $2 == ("*" f) { print $1; exit }' SHA256SUMS)
  fi

  local primary_url="${base_url}/${primary_file:-designos-${ver}-macos.dmg}"

  # Build artifacts array
  local artifacts_json=""
  if [ -f SHA256SUMS ]; then
    while read -r sha file; do
      file=${file#\*}
      [ -n "$file" ] || continue
      if [ -n "$artifacts_json" ]; then
        artifacts_json="${artifacts_json},"$'\n'
      fi
      artifacts_json="${artifacts_json}    {\"name\": \"${file}\", \"url\": \"${base_url}/${file}\", \"sha256\": \"${sha}\"}"
    done < SHA256SUMS
  fi

  cat > "$dist_dir/release-manifest.json" <<EOF
{
  "version": "${ver}",
  "channel": "${channel}",
  "published_at": "${published_at}",
  "download_url": "${primary_url}",
  "sha256": "${primary_sha256}",
  "release_notes": [
    "1:1 Figma UI3 Home, File Browser, and Left/Right Inspector parity",
    "Dual-Layer Atomic Self-Updater with SHA-256 verification and .bak rollback",
    "Cross-platform installers for macOS (.dmg + CLI), Windows (-setup.exe + .zip), and Linux (.deb + .tar.gz)"
  ],
  "min_figma_kiwi_version": 106,
  "artifacts": [
${artifacts_json}
  ]
}
EOF

  echo "Generated dist/SHA256SUMS and dist/release-manifest.json for v${ver} (${channel})"
}

if [ "$mode" = "generate-manifest" ]; then
  generate_manifest_and_sums "$version"
  exit 0
fi

uname_s=$(uname -s)
case "$uname_s" in
  Darwin*)
    if [ "$mode" = "universal-macos" ]; then
      if [ "$skip_build" -eq 0 ]; then
        for t in aarch64-apple-darwin x86_64-apple-darwin; do
          echo "Building release binaries for $t..."
          cargo build --release --locked --target "$t" -p designos-app -p designos-mcp -p designos-diff
        done
      fi
      "$root/scripts/package-macos.sh" "$version" \
        "$target_dir/aarch64-apple-darwin/release" \
        "$target_dir/x86_64-apple-darwin/release"
    else
      if [ "$skip_build" -eq 0 ]; then
        echo "Building release binaries for local macOS host..."
        cargo build --release --locked -p designos-app -p designos-mcp -p designos-diff
      fi
      "$root/scripts/package-macos.sh" "$version" "$target_dir/release" "$target_dir/nonexistent-single-arch"
    fi
    ;;

  Linux*)
    host_triple=$(rustc -vV | awk '/^host:/ { print $2 }')
    if [ "$skip_build" -eq 0 ]; then
      echo "Building release binaries for $host_triple..."
      cargo build --release --locked -p designos-app -p designos-mcp -p designos-diff
    fi
    "$root/scripts/package.sh" "$host_triple" "$version" "$target_dir/release"
    ;;

  MINGW*|MSYS*|CYGWIN*|Windows_NT*)
    host_triple=$(rustc -vV | awk '/^host:/ { print $2 }')
    if [ "$skip_build" -eq 0 ]; then
      echo "Building release binaries for $host_triple..."
      cargo build --release --locked -p designos-app -p designos-mcp -p designos-diff
    fi
    "$root/scripts/package.sh" "$host_triple" "$version" "$target_dir/release"
    ;;

  *)
    echo "build-release.sh: unsupported host OS: $uname_s" >&2
    exit 1
    ;;
esac

generate_manifest_and_sums "$version"
