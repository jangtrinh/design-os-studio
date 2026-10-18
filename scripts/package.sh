#!/usr/bin/env bash
# Package the release binaries for one target (Windows or Linux).
#
#   scripts/package.sh <target-triple> <version> [bin-dir]
#
# Expects all 4 binaries (`designos-studio`, `designos`, `designos-mcp`, `designos-diff`)
# already built in release mode. bin-dir defaults to target/<triple>/release, falling back
# to target/release.
#
# Writes into dist/:
#   Windows:
#     designos-<version>-<triple>.zip        (portable archive + install.ps1 / uninstall.ps1)
#     designos-<version>-<triple>-setup.exe  (NSIS per-user GUI installer, when makensis is available)
#     *.sha256
#   Linux:
#     designos-<version>-<triple>.tar.gz     (portable archive + install.sh / uninstall.sh / AppRun)
#     designos_<version>_<arch>.deb          (Debian/Ubuntu package, when dpkg-deb is available)
#     *.sha256
#   macOS:
#     delegates to scripts/package-macos.sh
set -euo pipefail

target=${1:?usage: package.sh <target-triple> <version> [bin-dir]}
version=${2:?usage: package.sh <target-triple> <version> [bin-dir]}
version=${version#v}
bin_dir=${3:-target/$target/release}
[ -d "$bin_dir" ] || bin_dir=target/release

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

case "$target" in
  *windows*) exe=.exe; fmt=zip ;;
  *apple-darwin*)
    exec "$root/scripts/package-macos.sh" "$version" "$bin_dir" "$bin_dir"
    ;;
  *) exe=; fmt=tar.gz ;;
esac

name=designos-$version-$target
mkdir -p "$root/dist"
stage=$root/dist/$name
rm -rf "$stage"
mkdir -p "$stage"

BINARIES=(designos-studio designos designos-mcp designos-diff)
for bin in "${BINARIES[@]}"; do
  if [ ! -f "$bin_dir/$bin$exe" ]; then
    echo "package.sh: missing binary $bin_dir/$bin$exe" >&2
    exit 1
  fi
  cp "$bin_dir/$bin$exe" "$stage/"
done

cp LICENSE-MIT LICENSE-APACHE README.md "$stage/"
mkdir -p "$stage/licenses"
cp "$root/crates/designos-fonts/assets/inter/OFL.txt" "$stage/licenses/Inter-OFL.txt"
cp "$root/crates/designos-fonts/assets/roboto-mono/OFL.txt" "$stage/licenses/RobotoMono-OFL.txt"
cp "$root/crates/designos-app/assets/icons/phosphor/LICENSE" "$stage/licenses/Phosphor-LICENSE.txt"
cp "$root/crates/designos-app/assets/icons/lucide/LICENSE" "$stage/licenses/Lucide-LICENSE.txt"

sha256_file() {
  local f=$1
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$f")" && sha256sum "$(basename "$f")" > "$(basename "$f").sha256")
  else
    (cd "$(dirname "$f")" && shasum -a 256 "$(basename "$f")" > "$(basename "$f").sha256")
  fi
  cat "$f.sha256"
}

case "$target" in
  *windows*)
    cp "$root/packaging/windows/install.ps1" "$stage/install.ps1"
    cp "$root/packaging/windows/uninstall.ps1" "$stage/uninstall.ps1"
    cp "$root/packaging/windows/installer.nsi" "$stage/installer.nsi"
    if [ -f "$root/assets/icon/designos.ico" ]; then
      cp "$root/assets/icon/designos.ico" "$stage/designos.ico"
    fi

    # Build NSIS setup.exe installer if makensis is available (pre-installed on GitHub windows-latest)
    setup_exe="$root/dist/designos-$version-$target-setup.exe"
    rm -f "$setup_exe"
    if command -v makensis >/dev/null 2>&1; then
      echo "Building Windows NSIS installer: $(basename "$setup_exe")"
      (cd "$stage" && makensis -DVERSION="$version" -DOUTFILE="$setup_exe" installer.nsi)
      sha256_file "$setup_exe"
    else
      echo "package.sh: makensis not found on PATH; skipping -setup.exe generation" >&2
    fi
    ;;

  *linux*)
    mkdir -p "$stage/icons"
    for size in 16 32 48 64 128 256 512; do
      cp "$root/assets/icon/png/icon_$size.png" "$stage/icons/$size.png"
    done
    cp "$root/packaging/linux/designos.desktop" "$stage/designos.desktop"
    cp "$root/packaging/linux/designos-mime.xml" "$stage/designos-mime.xml"
    cp "$root/packaging/linux/install.sh" "$stage/install.sh"
    cp "$root/packaging/linux/uninstall.sh" "$stage/uninstall.sh"
    cp "$root/packaging/linux/AppRun" "$stage/AppRun"
    chmod +x "$stage/install.sh" "$stage/uninstall.sh" "$stage/AppRun" \
      "$stage/designos-studio" "$stage/designos" "$stage/designos-mcp" "$stage/designos-diff"

    # Build Debian/Ubuntu .deb package if dpkg-deb is available
    if command -v dpkg-deb >/dev/null 2>&1; then
      case "$target" in
        x86_64*) deb_arch=amd64 ;;
        aarch64*|arm64*) deb_arch=arm64 ;;
        *) deb_arch=amd64 ;;
      esac
      deb_version=${version//-/\~}
      deb_name="designos_${version}_${deb_arch}.deb"
      deb_root="$root/dist/deb-work"
      rm -rf "$deb_root"
      mkdir -p \
        "$deb_root/DEBIAN" \
        "$deb_root/usr/bin" \
        "$deb_root/usr/share/applications" \
        "$deb_root/usr/share/mime/packages" \
        "$deb_root/usr/share/doc/designos/licenses"

      for bin in "${BINARIES[@]}"; do
        install -m 755 "$stage/$bin" "$deb_root/usr/bin/$bin"
      done

      sed \
        -e "s|^Exec=designos-studio|Exec=/usr/bin/designos-studio|" \
        -e "s|^TryExec=designos-studio|TryExec=/usr/bin/designos-studio|" \
        "$root/packaging/linux/designos.desktop" > "$deb_root/usr/share/applications/designos.desktop"
      chmod 644 "$deb_root/usr/share/applications/designos.desktop"

      install -m 644 "$root/packaging/linux/designos-mime.xml" "$deb_root/usr/share/mime/packages/designos.xml"
      install -m 644 LICENSE-MIT LICENSE-APACHE README.md "$deb_root/usr/share/doc/designos/"
      install -m 644 "$stage/licenses/"* "$deb_root/usr/share/doc/designos/licenses/"

      for size in 16 32 48 64 128 256 512; do
        icon_dir="$deb_root/usr/share/icons/hicolor/${size}x${size}/apps"
        mkdir -p "$icon_dir"
        install -m 644 "$root/assets/icon/png/icon_$size.png" "$icon_dir/designos.png"
      done

      installed_kb=$(du -sk "$deb_root/usr" | awk '{print $1}')
      cat > "$deb_root/DEBIAN/control" <<EOF
Package: designos
Version: $deb_version
Section: graphics
Priority: optional
Architecture: $deb_arch
Installed-Size: $installed_kb
Depends: libc6, libgl1, libx11-6, libwayland-client0, libxkbcommon0, libfontconfig1
Maintainer: design:os studio contributors <opensource@designos.dev>
Homepage: https://github.com/jangtrinh/design-os-studio
Description: design:os studio — vector, UI, and spec design studio
 Fast, open-source Figma-compatible vector and UI design studio in Rust,
 shipping the GUI studio (designos-studio), headless CLI (designos),
 MCP server (designos-mcp), and visual diff tool (designos-diff).
EOF
      chmod 755 "$deb_root/DEBIAN"
      chmod 644 "$deb_root/DEBIAN/control"

      deb_out="$root/dist/$deb_name"
      rm -f "$deb_out"
      dpkg-deb --root-owner-group --build "$deb_root" "$deb_out"
      rm -rf "$deb_root"
      echo "packaged dist/$deb_name"
      sha256_file "$deb_out"
    fi
    ;;
esac

cd "$root/dist"
if [ "$fmt" = zip ]; then
  archive=$name.zip
  rm -f "$archive"
  if command -v zip >/dev/null 2>&1; then
    zip -qr "$archive" "$name"
  elif command -v 7z >/dev/null 2>&1; then
    7z a -tzip -bso0 "$archive" "$name"
  else
    python3 -m zipfile -c "$archive" "$name" 2>/dev/null || python -m zipfile -c "$archive" "$name"
  fi
else
  archive=$name.tar.gz
  rm -f "$archive"
  tar -czf "$archive" "$name"
fi
rm -rf "$stage"

sha256_file "$root/dist/$archive"
echo "packaged dist/$archive"
