#!/usr/bin/env bash
# Package design:os studio for macOS: universal (arm64 + x86_64) or single-arch native designos.app,
# shipped as a .dmg (with /Applications shortcut + install-cli.command) and a .tar.gz.
#
#   scripts/package-macos.sh <version> [arm64-bin-dir] [x86_64-bin-dir]
#
# Supports:
#   1. Universal build when both arm64-bin-dir and x86_64-bin-dir contain the 4 binaries
#      -> dist/designos-<version>-macos.dmg + dist/designos-<version>-macos-universal.tar.gz
#   2. Single-arch host build when only one target dir (or target/release) contains the 4 binaries
#      -> dist/designos-<version>-macos.dmg + dist/designos-<version>-macos-<arch>.tar.gz
#
# All 4 binaries are bundled inside designos.app/Contents/MacOS/:
#   designos-studio, designos, designos-mcp, designos-diff
set -euo pipefail

version=${1:?usage: package-macos.sh <version> [arm64-bin-dir] [x86_64-bin-dir]}
version=${version#v}
arm_dir=${2:-target/aarch64-apple-darwin/release}
x64_dir=${3:-target/x86_64-apple-darwin/release}
host_dir=target/release

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
dist=$root/dist
work=$dist/macos-work
rm -rf "$work"
mkdir -p "$work/stage" "$dist"

# CFBundleVersion must be dotted numbers; strip pre-release/build suffix (0.2.0-rc.1 -> 0.2.0).
short_version=$version
bundle_version=${version%%[-+]*}

app=$work/stage/designos.app
macos_dir=$app/Contents/MacOS
res_dir=$app/Contents/Resources
mkdir -p "$macos_dir" "$res_dir"

BINARIES=(designos-studio designos designos-mcp designos-diff)
arch_suffix="universal"

# 1. Assemble Universal (lipo) or Single-Arch binaries.
for bin in "${BINARIES[@]}"; do
  if [ -f "$arm_dir/$bin" ] && [ -f "$x64_dir/$bin" ]; then
    lipo -create -output "$macos_dir/$bin" "$arm_dir/$bin" "$x64_dir/$bin"
  elif [ -f "$arm_dir/$bin" ]; then
    cp "$arm_dir/$bin" "$macos_dir/$bin"
    arch_suffix="arm64"
  elif [ -f "$x64_dir/$bin" ]; then
    cp "$x64_dir/$bin" "$macos_dir/$bin"
    arch_suffix="x86_64"
  elif [ -f "$host_dir/$bin" ]; then
    cp "$host_dir/$bin" "$macos_dir/$bin"
    detected_arch=$(lipo -archs "$macos_dir/$bin" 2>/dev/null || uname -m)
    case "$detected_arch" in
      *"arm64"*"x86_64"*|*"x86_64"*"arm64"*) arch_suffix="universal" ;;
      arm64|aarch64) arch_suffix="arm64" ;;
      x86_64) arch_suffix="x86_64" ;;
      *) arch_suffix="$detected_arch" ;;
    esac
  else
    echo "package-macos.sh: missing binary '$bin' (checked $arm_dir, $x64_dir, and $host_dir)" >&2
    exit 1
  fi
  chmod 755 "$macos_dir/$bin"
  lipo -info "$macos_dir/$bin"
done

# 2. Icon: build the .icns from the committed iconset; fall back to the committed .icns.
iconset=$root/assets/icon/designos.iconset
if command -v iconutil >/dev/null 2>&1 && [ -d "$iconset" ]; then
  iconutil -c icns -o "$res_dir/designos.icns" "$iconset"
elif [ -f "$root/assets/icon/designos.icns" ]; then
  cp "$root/assets/icon/designos.icns" "$res_dir/designos.icns"
elif [ -f "$root/assets/icon/designos-studio.icns" ]; then
  cp "$root/assets/icon/designos-studio.icns" "$res_dir/designos.icns"
else
  echo "package-macos.sh: warning: no .icns icon found" >&2
fi

# 3. Info.plist from the template.
sed -e "s/@SHORT_VERSION@/$short_version/g" -e "s/@BUNDLE_VERSION@/$bundle_version/g" \
  "$root/packaging/macos/Info.plist.in" > "$app/Contents/Info.plist"
if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$app/Contents/Info.plist"
fi
printf 'APPL????' > "$app/Contents/PkgInfo"

# Keep embedded asset notices inside the app, including when installed from the DMG.
mkdir -p "$res_dir/licenses"
cp "$root/crates/designos-fonts/assets/inter/OFL.txt" "$res_dir/licenses/Inter-OFL.txt"
cp "$root/crates/designos-fonts/assets/roboto-mono/OFL.txt" "$res_dir/licenses/RobotoMono-OFL.txt"
cp "$root/crates/designos-app/assets/icons/phosphor/LICENSE" "$res_dir/licenses/Phosphor-LICENSE.txt"
cp "$root/crates/designos-app/assets/icons/lucide/LICENSE" "$res_dir/licenses/Lucide-LICENSE.txt"

# 4. Ad-hoc sign each helper binary first, then sign and verify the .app bundle.
for bin in designos-diff designos-mcp designos designos-studio; do
  codesign --force --sign - "$macos_dir/$bin"
done
codesign --force --deep --sign - "$app"
codesign --verify --deep --strict --verbose=2 "$app"

# 5. Disk image: designos.app, Applications symlink, install-cli.command, and Read me first.txt.
stage=$work/stage
ln -s /Applications "$stage/Applications"
cp "$root/packaging/macos/Read me first.txt" "$stage/Read me first.txt"
if [ -f "$root/packaging/macos/install-cli.command" ]; then
  cp "$root/packaging/macos/install-cli.command" "$stage/install-cli.command"
  chmod 755 "$stage/install-cli.command"
fi

dmg=$dist/designos-$version-macos.dmg
rm -f "$dmg"
# hdiutil occasionally fails with "Resource busy" on CI runners; retry a few times.
for attempt in 1 2 3; do
  if hdiutil create -volname "design:os studio" -srcfolder "$stage" -ov -format UDZO "$dmg"; then
    break
  fi
  [ "$attempt" = 3 ] && { echo "package-macos.sh: hdiutil failed" >&2; exit 1; }
  sleep 5
done

# 6. Tarball (.tar.gz) for CLI, self-updater, and scripting use.
name=designos-$version-macos-$arch_suffix
tar_stage=$work/$name
mkdir -p "$tar_stage"
cp -R "$app" "$tar_stage/designos.app"
if [ -f "$root/packaging/macos/install-cli.command" ]; then
  cp "$root/packaging/macos/install-cli.command" "$tar_stage/install-cli.command"
  chmod 755 "$tar_stage/install-cli.command"
fi
cp LICENSE-MIT LICENSE-APACHE README.md "$tar_stage/"
tarball=$dist/$name.tar.gz
rm -f "$tarball"
COPYFILE_DISABLE=1 tar -C "$work" -czf "$tarball" "$name"

# 7. Checksums.
for f in "$dmg" "$tarball"; do
  (cd "$dist" && shasum -a 256 "$(basename "$f")" > "$(basename "$f").sha256")
  cat "$f.sha256"
done

rm -rf "$work"
echo "packaged dist/$(basename "$dmg") and dist/$(basename "$tarball")"
