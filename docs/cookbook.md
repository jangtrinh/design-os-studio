# `design-os-studio` Prompt & CLI Cookbook

Copy-paste recipes for **DESIGN:OS Studio** (`skills/design-os-studio/SKILL.md`, `designos-studio`, `designos`, `designos-mcp`, and `designos-diff`).

---

## 1. Generate & Roundtrip `.spec.json` -> `.fig` v106 Binary

> "Apply `screen.spec.json` onto the DESIGN:OS Studio canvas, export a verified Figma `.fig` v106 binary archive to `out.fig`, and capture a 1600x1000 GPU screenshot."

```bash
# 1. Generate or update a screen from a declarative NodeSpec JSON with Auto-Layout 5.0
designos app.designos --apply-spec screen.spec.json

# 2. Save any document (.designos or .fig) directly to a standard Figma .fig v106 binary archive.
#    The path must end in .fig and must not exist yet; add --overwrite to replace an existing regular
#    file. A symbolic link is never written through.
designos app.designos --export-fig out.fig

# 3. Export any canvas frame back to a clean, diffable NodeSpec JSON
designos app.designos --select "EaseUI — Agent-First Spec Console" --export-spec out.spec.json
```

---

## 2. Inspect Specs, Agentic PTY Chat & Canvas Pin-Comments

> "Open the 11-stage PRODUCT:OS Spec Driven v2 Reader (`--panel specs`), inspect all User Stories (`US-01..US-15`), Gherkin scenarios, and `g1..g5` gates, and resolve open canvas pin-comments."

```bash
# Launch the 11-stage PRODUCT:OS Spec Driven v2 Reader
designos-studio --open-id <file-id> --panel specs

# Launch the native Agentic PTY Chat UI wrapper (Claude Code · Codex · Antigravity CLI)
# Keyboard shortcut inside Studio: Cmd+Alt+J (macOS) / Ctrl+Alt+J (Linux/Windows) moves focus to the Agents composer (or terminal) and back to the canvas.
designos-studio --open-id <file-id> --rail agents

# Read human feedback pins from the Canvas (resolved to exact NodeId + parent Screen Frame)
designos app.designos --comments-json

# Pin a comment, reply to human feedback, and mark the thread resolved
designos app.designos --select "Apply Spec CTA" --add-comment "Switch CTA to Syncing state while agent runs"
designos app.designos --reply-comment t1 --message "Updated spec and auto-reloaded on Canvas!" --resolve
```

- **Terminal-Focused Document Undo / Redo**: While an agent terminal has keyboard focus, `Cmd+Z` and `Shift+Cmd+Z` / `Cmd+Y` on macOS (or `Ctrl+Z` and `Ctrl+Shift+Z` / `Ctrl+Y` on Windows and Linux for agent tabs whose CLI does not reserve `Ctrl+Z`; `Shell` tabs keep `Ctrl+Z` for the shell) undo and redo document edits one step per agent tool call without leaving the terminal.

---

## 3. Headless GPU Screenshot & Visual QA Automation (`13` Studio Surfaces)

> "Run the headless GPU visual proof capture suite across all 13 DESIGN:OS Studio surfaces (`Hero`, `Agents Chat`, `Assets`, `Actions Cmd+K`, `Dev Mode`, `Variables`, `Components`, `Prototype`, `Main Menu`, `Color Picker`, `Agent Spec`, `Overview LOD`, `Home`) and verify SSIM parity."

```bash
# Re-capture the 13 deterministic studio screenshots into docs/images/ from a .designos copy of the Untitled UI PRO v8.0 kit
DESIGNOS_PROOF_DOC=/path/to/untitled-ui-kit.designos ./scripts/capture-visual-proof.sh

# Render a specific frame at 60% zoom using the Metal/Vulkan/DX12 GPU pipeline
designos-studio --open-id <file-id> --select "IAM · Audit log" --zoom 60 --gpu --size 1600x1000 --screenshot docs/images/designos-hero.png

# Render Dev Mode with React code generation and box model inspector open
designos-studio --open-id <file-id> --select "API Docs" --zoom 100 --tab dev --dev-lang react --gpu --size 1600x1000 --screenshot docs/images/designos-devmode.png

# Compare two renders with perceptual SSIM + anti-aliasing tolerance
designos-diff compare reference.png docs/images/designos-hero.png --out diff.png
```

---

## 4. Diagnostics & Cross-Platform Release Packaging

```bash
# Run deterministic offline repository & skill self-check
python3 scripts/self_check.py

# Run full workspace Rust test suite (823 tests)
cargo test --workspace

# Build & package native installers (.dmg, -setup.exe, .deb) + SHA256SUMS into dist/
./scripts/build-release.sh 0.2.0 --local-host
```
