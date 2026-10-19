# Installing **design:os studio**

Download the latest package for your platform from [GitHub Releases](https://github.com/jangtrinh/design-os-studio/releases) or build a local release package directly from source.

> **Platform support:** releases are verified on macOS (Apple Silicon). The Windows and Linux packages are built by the same release workflow and tested by their users. Please report anything that breaks.

Every release ships all **4 binaries**:
- `designos-studio` — Native GPU (`wgpu`) + CPU (`tiny-skia`) Studio GUI
- `designos` — GUI + headless CLI renderer, exporter, and batch processor
- `designos-mcp` — Model Context Protocol (MCP) server for Claude Code, Codex, and Antigravity
- `designos-diff` — Visual & `.fig` structural diff CLI tool

Each release also attaches `SHA256SUMS` and `release-manifest.json`. Verify a download against `SHA256SUMS` before you open it.

---

## 1. macOS (Apple Silicon & Intel)

### Option A: Drag-and-Drop `.dmg` Installer (Recommended)

1. Download `designos-<version>-macos.dmg` from [GitHub Releases](https://github.com/jangtrinh/design-os-studio/releases).
2. Double-click the `.dmg` and drag **`designos.app`** into **`Applications`**.
3. *(Optional — CLI & Gatekeeper Setup)* Double-click **`install-cli.command`** inside the mounted DMG window. This script:
   - Clears the macOS Gatekeeper quarantine flag (`xattr -dr com.apple.quarantine /Applications/designos.app`)
   - Symlinks `designos-studio`, `designos`, `designos-mcp`, and `designos-diff` into `~/.local/bin` (or `/usr/local/bin`)

The app is ad-hoc signed, not notarized. If you launch `designos.app` without running `install-cli.command`, macOS asks for a one-time approval: open System Settings → Privacy & Security and click **Open Anyway**, or clear the quarantine attribute once in Terminal:

```bash
xattr -dr com.apple.quarantine /Applications/designos.app
```

### Option B: Tarball Archive (`.tar.gz`)

```bash
tar -xzf designos-<version>-macos-universal.tar.gz
cp -R designos-<version>-macos-universal/designos.app /Applications/
./designos-<version>-macos-universal/install-cli.command
```

---

## 2. Windows (x64)

Releases ship a portable `.zip` with a PowerShell installer. The binaries are unsigned, so Windows SmartScreen may warn (**More info → Run anyway**). An NSIS installer is built only when NSIS (`makensis`) is available on the build machine; the release runner does not have it, so it is not part of the release downloads.

1. Download and extract `designos-<version>-x86_64-pc-windows-msvc.zip`.
2. Run the bundled PowerShell installer. It performs a **per-user installation** under `%LOCALAPPDATA%\Programs\designos` (no Administrator prompt), copies the 4 binaries to `%LOCALAPPDATA%\Programs\designos\bin`, adds them to your User `PATH`, creates Start Menu and Desktop shortcuts, and registers `.designos` / `.fig` file associations:

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

To uninstall later:

```powershell
powershell -ExecutionPolicy Bypass -File "$env:LOCALAPPDATA\Programs\designos\uninstall.ps1"
```

---

## 3. Linux (x64)

### Option A: Ubuntu / Debian Package (`.deb`, Recommended on Debian-based distros)

```bash
sudo apt install ./designos_<version>_amd64.deb
```

This installs `/usr/bin/{designos-studio,designos,designos-mcp,designos-diff}`, the desktop launcher (`/usr/share/applications/designos.desktop`), hicolor icons, and `.designos` / `.fig` MIME types.

### Option B: Universal Linux Tarball (`install.sh`, No Root Required)

```bash
tar -xzf designos-<version>-x86_64-unknown-linux-gnu.tar.gz
cd designos-<version>-x86_64-unknown-linux-gnu
./install.sh
```

By default, `./install.sh` installs under `~/.local/bin` and `~/.local/share` (override with `PREFIX=/usr/local ./install.sh`). To remove:

```bash
./uninstall.sh
```

The binaries link the glibc of the build machine, so they run on that Linux distribution or newer.

---

## 4. Building & Packaging Releases Locally (`scripts/build-release.sh`)

To compile all 4 release binaries (`designos-studio`, `designos`, `designos-mcp`, `designos-diff`) and build the packages for your platform + `dist/release-manifest.json` + `dist/SHA256SUMS` in one command:

```bash
# Build & package for your current host OS (macOS .dmg + .tar.gz, Linux .deb + .tar.gz, Windows .zip)
./scripts/build-release.sh 0.5.0 --local-host

# Build & package a Universal (Apple Silicon + Intel) macOS release
./scripts/build-release.sh 0.5.0 --universal-macos

# Package already-built release binaries in target/release without recompiling
./scripts/build-release.sh 0.5.0 --local-host --skip-build
```

Outputs are written to `dist/`:
- `dist/designos-<version>-macos.dmg` & `dist/designos-<version>-macos-*.tar.gz` (macOS)
- `dist/designos-<version>-<target>.zip` (Windows, plus an NSIS installer when `makensis` is installed)
- `dist/designos_<version>_<arch>.deb` & `dist/designos-<version>-<target>.tar.gz` (Linux)
- `dist/release-manifest.json` (version, channel, and the name, download URL, and SHA-256 of each package)
- `dist/SHA256SUMS`

---

## 5. GPU Rendering

**design:os studio** renders with `wgpu` (Metal on macOS, Direct3D 12 / Vulkan on Windows, Vulkan / OpenGL ES on Linux) and uses the deterministic `tiny-skia` CPU renderer when the GPU cannot be initialised or when **Use GPU rendering** is off. Only the Metal path on macOS is verified by the maintainers; the other backends are tested by their users. Pass `--gpu` on the CLI to force GPU rendering in headless `--screenshot` mode.
