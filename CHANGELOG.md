# Changelog (`design-os-studio`)

All notable changes to `design-os-studio` (**DESIGN:OS Studio**) are documented in this file.

## [0.3.0] - 2026-10-17

Pack **Paste & Styles** ([milestone v0.3](https://github.com/jangtrinh/design-os-studio/milestone/1)).

### Added
- **Paste from Figma (#27).**
  - Copy a selection in Figma desktop and press Cmd+V on the canvas.
  - The layers land centered in the view as one undo step, images included.
- **Effect and grid styles (#27).**
  - Create, apply, detach and rename them from the Effects and Layout guide sections.
  - They also show under local styles.
  - `.fig` files bring their effect and grid styles with them.
- **Edit a style from the style picker (#44).** Changing a style's values updates every layer that uses it, as fill or as stroke, in one undo step. Library styles stay read-only.
- **PDF export (#27).** Selected frames export as one multi-page PDF from the Export panel. Pages are raster images for now.
- **Auto layout (#27).**
  - Space around and Space evenly.
  - Wrap in the vertical direction, with wrapped lines that can spread.
- **Text wrap Balance and Pretty (#27).**
- **Variables (#27).**
  - Paint and effect opacity can be bound to a number variable.
  - Variables keep their code syntax (Web, Android, iOS), which Dev Mode prints the way Figma does.
- **Dev Mode: UIKit and Android XML (#27).**
- **Clean captures:** `--no-toast` and `--no-caption` for `--screenshot` (#13).

### Fixed
- **Agent edits keep the open document intact (#28).**
  - `designos_apply_spec` and other edits made through the bundled `designos-mcp` server no longer replace the open document. On large files, the other frames used to become empty boxes and the Text styles list emptied.
  - Edits are replayed as node operations: one undo step per batch, the new layer is selected, and the view stays where it was.
  - Agent edits wait while you drag or edit text, then land, and they survive switching documents.
  - Undo through the MCP server is refused once the canvas has reloaded, so it can never remove your own layers. Undo in Studio works as before.
  - Update `designos-mcp` and Studio together; the bundled server matches the app.
- **Files with broken numbers open again (#42, #6).**
  - A value that was not a number (NaN or infinity) used to be saved as `null`. The file then refused to open with `invalid type: null, expected f32`.
  - Such values are now never written. Files that already contain them open, with each one replaced by a neutral value.
- **Large pages render at low zoom (#13).** The GPU canvas renders each frame in its own submission. Pages such as a 68,698-layer `.fig` at 3% zoom no longer show an empty canvas.
- **Untrusted files and saves (#1).**
  - `.fig`, kiwi and `.pen` decoding is bounded (total inflate budget, entry and depth caps).
  - Saves are atomic (temp file in the same folder, fsync, rename).
  - Documents with dangling or cyclic layer references are repaired on load instead of crashing; the MCP `open_file` tool refuses repaired documents.
- **The app reports its real version.**
  - The workspace version was still `0.1.0` in v0.2.0, so the About dialog and the updater misreported it.
  - The release workflow now fails if the tag and `Cargo.toml` disagree.

### Changed
- **Variable references in CSS, React and Tailwind (#27)** keep the variable's case and print a fallback, for example `var(--Color-Primary, #0D99FF)`.
- **Idle full-quality GPU refine (#1).**
  - After 250 ms without changes, the canvas renders one uncapped frame (masks, blur, blend modes, group opacity), bounded by a 512 MiB VRAM budget.
  - Any interaction returns to the capped interactive path.
- **Lower memory on `.fig` import.** Peak RSS on large files is about 6.4 GB lower, and Phase 2 engine optimizations were added.
- **Removed UI that did nothing (#1).**
- **The README and the site open with the 70-second promo film (#48).**
- **Releases are verified on macOS.** A failed Windows or Linux build no longer blocks the macOS release; those packages are attached when their builds pass.

## [0.2.0] - 2026-10-15

### Added
- **Cross-Platform Installers (`DIR-22`)**: Added one-command release builder (`scripts/build-release.sh`) producing macOS `.dmg` (`designos.app` + `install-cli.command`) + `.tar.gz`, Windows NSIS `-setup.exe` + `.zip` (`install.ps1`/`uninstall.ps1`), Linux Debian `.deb` + portable `.tar.gz` (`install.sh`), `dist/release-manifest.json`, and `dist/SHA256SUMS`.
- **Native Agentic PTY Chat UI Wrapper (`DIR-21`)**: Added structured User/Assistant chat turn bubbles (`AgentChatTurn` / `AgentChatRole`) inside the `Agents` Left Rail (`RailItem::Agents`) wrapping live `portable-pty` + `vt100` sessions for Claude Code (`claude`), Codex (`codex`), and Antigravity CLI (`agy`) with inline `NodeSpec` detection and 1-click Canvas apply.
- **100% Unified Phosphor Icons System (`DIR-19`)**: Unified all UI3 chrome, toolbars, left rail, inspector, and Spec Reader surfaces on official Phosphor Icons (`@phosphor-icons/core` `256x256` SVG rasterized via `resvg`).
- **11-Stage `PRODUCT:OS Spec Driven v2` Reader (`DIR-18`)**: Full in-app Spec Reader (`--panel specs` / `--rail specs`) rendering `US-01..US-15` User Stories, Gherkin `Given/When/Then` scenarios, and `g1..g5` verification gates.
- **1:1 Figma `126.9.13` UI3 & `.fig` v106 Parity (`387 / 387 — 100.00%`)**: Complete visual chrome, typography, Auto-Layout 5.0, Variables, Components, Prototype flows, Dev Mode codegen, and native `.fig` v106 binary read/write parity verified by `823` automated tests.
- **AI-Ready Repository Delivery & 9-Tier GitHub Pages (`DIR-23`)**: Added `.claude-plugin`, `.codex-plugin`, `.agents/plugins`, `.factory-plugin`, `skills/design-os-studio/SKILL.md`, `/doctor`, `scripts/self_check.py`, 12-section editorial `README.md`, and 9-tier `docs/index.html` with 15 visual proof captures.
