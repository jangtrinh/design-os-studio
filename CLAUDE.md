# DESIGN:OS Studio (`design-os-studio`) — Agent Instructions (`CLAUDE.md`)

Native GPU-accelerated Rust design canvas, `.fig` / `.designos` / `.designos` editor, and headless MCP server (`designos-mcp` / `designos-mcp`) for the **DESIGN:OS / EaseUI** ecosystem.

> **Before starting any work**, read:
> 1. `.project-agent.md` — product identity, EaseUI BrainWave 2.0 theme tokens, and verification commands.
> 2. `CONTEXT.md` — canonical domain glossary (`Document`, `TileView::rev`, `SceneIndex`, Compact Native Format).
> 3. `.brv/context-tree/session-handoffs/latest.md` — latest session state, verified metrics, and active invariants.
> 4. `.brv/context-tree/architecture/crate-map.md` & `.brv/context-tree/architecture/figma-vs-designos-engine.md` — crate responsibilities, hot paths, and Figma engine mapping.

---

## 1. Required Skills (Mandatory)

Every agent working on `design-os-studio` MUST load and follow:
- **`es:rust`** — Rust 2021 ownership, borrowing, zero-allocation hot paths, `parking_lot` / `egui` lock discipline, and deterministic error handling.
- **`es:rust-gpu-canvas`** — `wgpu` Metal/Vulkan/GL rendering architecture, `egui-wgpu` callback lifecycle, `lyon` tessellation budgeting, spatial culling, and CPU/GPU parity.
- **`es:astra-work`** — Outer execution loop, outcome contract, and post-mutation verification with real evidence.
- **`es:fable-thinking`** — Epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), falsification-first debugging, and prediction discipline.

---

## 2. Workspace Crate Map (11 Crates)

| Crate | Path / Binary | Role & Primary Responsibility |
|---|---|---|
| **`designos-doc`** | `crates/designos-doc` | Core document model (`Document`, `Node`, `NodeId`, `NodeKind`), compact JSON serializer (`serde_images` `"hex:..."` + `skip_serializing_if`), styles, variables, components, prototyping, and undo/redo history. |
| **`designos-fig`** | `crates/designos-fig` | Figma `.fig` (`fig-kiwi` binary container + DEFLATE/Zstd) and Figma REST JSON importer/exporter, VectorNetwork converter, and component instance expander. |
| **`designos-render`** | `crates/designos-render` | Deterministic CPU reference renderer (`tiny-skia 0.11`), `SceneIndex` world-bounds spatial & paint-order index, `TileCache`, boolean path ops (`i_overlay`), and `cosmic-text` shaping. |
| **`designos-gpu`** | `crates/designos-gpu` | `wgpu` GPU canvas renderer: `SceneCompiler` (`compile.rs`), zoom-bucket `GeomCache` (`geom.rs`, `lyon_tessellation`), `PaintCache` (`paintconv.rs`), and multi-pass `Renderer` (`exec.rs`, custom WGSL shaders). |
| **`designos-app`** | `crates/designos-app` (`designos-studio`, `designos`) | Desktop editor UI (`egui 0.31` + `eframe 0.31`), EaseUI BrainWave 2.0 theme (`theme.rs`), GPU/tile canvas viewport (`canvas/`), virtualized Layers panel (`panels/layers.rs`), Home library, and CLI (`cli.rs`). |
| **`designos-library`** | `crates/designos-library` | On-disk projects library (`store.rs`): `.designos`/`.designos`/`.fig` storage, 8 KB `file_kind` header probe, single-pass `import_file_with_doc`, thumbnails, version history, and published libraries. |
| **`designos-fonts`** | `crates/designos-fonts` | Bundled Inter font + on-demand Fontsource/Google Fonts HTTP fetcher, disk cache, script fallback, and zero-clone `ensure_font_requests_with`. |
| **`designos-svg`** | `crates/designos-svg` | SVG importer mapping `usvg` trees onto `designos_doc::Node` hierarchies with paint, text, and filter conversion. |
| **`designos-export`** | `crates/designos-export` | PNG/JPEG/SVG/PDF document exporter and Dev Mode codegen (`CSS`, `Tailwind`, `React`, `SwiftUI`, `Jetpack Compose`). |
| **`designos-mcp`** | `crates/designos-mcp` (`designos-mcp`, `designos-mcp`) | Headless stdio JSON-RPC MCP server (`rmcp`) exposing document inspection, creation, layout, export, and library operations to AI agents. |
| **`designos-cli` / `designos-diff`** | `crates/designos-diff` (`designos-diff` CLI) + `crates/designos-app/src/cli.rs` | CLI harness & pixel-diff test suite comparing CPU/GPU renders against reference screenshots (`crates/designos-diff`) plus desktop CLI flags (`--screenshot`, `--open-id`, `--gpu`, `--fixture`). |

---

## 3. Build, Test & Run Commands

Always prepend `$HOME/.cargo/bin` to `PATH` before invoking `cargo` or `rustc`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"

# Build release binaries (desktop editor + MCP server)
cargo build --release -p designos-app -p designos-mcp
cargo build --release -p designos-app

# Sync release binaries into the macOS app bundle (always all three: Studio and designos-mcp
# share the live edit-log format, and an older designos-mcp still overwrites the open document)
cp target/release/designos-studio "target/release/bundle/DESIGN-OS Studio.app/Contents/MacOS/designos-studio"
cp target/release/designos "target/release/bundle/DESIGN-OS Studio.app/Contents/MacOS/designos"
cp target/release/designos-mcp "target/release/bundle/DESIGN-OS Studio.app/Contents/MacOS/designos-mcp"

# Launch the packaged macOS app bundle
open "target/release/bundle/DESIGN-OS Studio.app"

# Launch directly into the 112,474-node (723,136 Kiwi records) "VSF - PCP" enterprise benchmark fixture
# (or `--open-id 18de8bff2547cd200000` for the full 31-page 1,082,324-node import)
"target/release/bundle/DESIGN-OS Studio.app/Contents/MacOS/designos-studio" --open-id 18de678a186512c80000

# Run verification suites
cargo test --workspace --lib
cargo test -p designos-gpu
cargo test -p designos-mcp
```

---

## Git Workflow (`main` is protected)

Since 2026-10-17 `main` is protected by a repository ruleset. Nobody bypasses it: not the owner, and not any agent runtime (Claude, Codex, Grok, Gemini).

- Never push to `main`. Work on a branch or worktree, push it, and open a pull request.
- To merge, a PR needs green `Test (macos-14)`, `Clippy` and `Rustfmt` on a branch that is up to date with `main`. Run `gh pr update-branch <n> --rebase`, then wait for CI. Ubuntu and Windows CI also run but do not block, because releases are verified on macOS.
- Merge with squash only (`gh pr merge <n> --squash`). The head branch is deleted on merge, and `main` keeps a linear history.
- Release tags `v*` cannot be moved or deleted once pushed. Tag the squash commit of the release PR (see `docs/releasing.md`).

## Marketing Videos (owner HARD RULE, 2026-10-17)

- Every piece of demo UI in a project video (launch and promo films, micro clips, GIFs, long-form captures) comes from the Untitled UI PRO VARIABLES v8.0 kit, `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`, opened in Studio. Never show a blank or self-built demo file, another kit, or a client file (e.g. VSF - PCP).
- Work from a copy (a `.fig` import saved as `.designos`) so takes never save over the original. Feature demos may edit that copy through Studio or `designos-mcp`. Extra demo frames go on a scratch page inside it.
- The kit's default brand colour is purple. Use neutral frames, or re-theme through the kit's own colour variables (Purple Ban). Never recolour pixels, and show no photos of people.
- Credit "Untitled UI PRO v8.0 kit by Untitled UI" in the end card.

## 4. Non-Negotiable Performance & Safety Invariants

1. **NEVER call `doc.to_json()` inside `doc_hash` or on the UI thread (`crates/designos-app/src/home/session.rs`):**
   - `doc_hash(doc: &Document) -> u64` MUST use the zero-allocation `HashSerializer` streaming directly into `DefaultHasher` (and sampling image byte headers/tails). Allocating a JSON string on a 112,474-node (723,136 Kiwi records) document stalls the UI thread and spikes memory by gigabytes.
   - Viewport-only operations (pan, zoom, fit-to-screen, marquee selection) MUST NEVER call `app.changed()` or increment `TileView::rev`.

2. **NEVER call `ctx.load_texture` inside `ctx.data_mut` (`crates/designos-app/src/icons.rs` & all `egui` code):**
   - `egui::Context::data_mut` acquires a write lock on `egui`'s internal `parking_lot::RwLock<ContextImpl>`. Calling `ctx.load_texture(...)` while that closure is active attempts to re-lock `ContextImpl`, causing an immediate non-reentrant `RwLock` self-deadlock on Frame 1. Always release `ctx.data_mut` before calling `ctx.load_texture`, then insert the handle in a separate `ctx.data_mut` call.

3. **PRESERVE the `interactive()` guard in `crates/designos-gpu/src/compile.rs`:**
   - `fn interactive(&self) -> bool { self.cull && self.geom.budget.is_some() }` distinguishes live interactive viewport rendering (`budget = Some(8ms)`) from headless reference rendering (`budget = None`).
   - All interactive fast-paths (pre-resolve `< 0.75px` sub-pixel culling, `< 3.5px` text greeking, hardware scissor fallback on rounded clips, and offscreen `Program` caps) MUST stay gated behind `self.interactive()` so `cargo test -p designos-gpu` headless CPU-vs-GPU pixel-diff tests remain bit-exact.

4. **CAP offscreen GPU `Program` allocations (`self.progs.len() < 64`) and reuse `FrameKey` (`crates/designos-app/src/canvas/gpu.rs`):**
   - Unbounded offscreen render passes (`Program`s) on 112,474-node (723,136 Kiwi records) pages exhaust `egui-wgpu`'s Metal staging ring buffer and crash the GPU driver. Keep offscreen layer/mask/clip/blend `Program` creation capped at `< 64` in `interactive()` mode (falling back to hardware scissor clips).
   - Exception (owner ruling 2026-10-16, ratified 2026-10-17 after a live Metal check): when the view is idle, `GpuView::plan` runs exactly ONE compile with `CompileOptions.uncap_programs = true`, caches it under the same `FrameKey` (`refined = true`), and from then on re-blits with `Cb { frame: None }`. Masks, blur, blend modes and group opacity render at full quality at rest. Any viewport or document change goes back to the capped interactive compile.
   - Idle = the `FrameKey` stayed unchanged for `IDLE_REFINE_DELAY` (250 ms of `ctx.input(|i| i.time)`, no timer thread), `stale_hits == 0`, no pointer down, and no scroll or zoom delta in the last frame (a delta restarts the delay, so a trackpad pan or momentum scroll never triggers the refine). `plan` returns `FramePlan::Wait`, which re-blits the cache and requests ONE `request_repaint_after` for the remaining delay; after the refine every idle frame is `Reuse` (`Cb { frame: None }`) with no repaint request (0% idle CPU).
   - VRAM ceiling (owner ruling 2026-10-16, replaces any program-count limit for the refine): the uncapped compile accounts the GPU bytes of every offscreen target (`program_bytes`: pooled extent x `FORMAT` 4 B x (`MSAA` 4 + 1 resolve), plus blur/displace/backdrop scratch targets; formula mirrors `resources.rs` `Pool`) and opens a new `Program` only while the running total stays within `CompileOptions.refine_vram_budget_bytes` (default 512 MiB, `DEFAULT_REFINE_VRAM_BUDGET_BYTES`). Past the budget it degrades exactly like the capped path (scissor clip instead of a mask program, effect or layer skipped). Capped interactive and headless (`budget = None`) compiles never consult the budget.
   - In `canvas::gpu::paint`, when `last_key == Some(key) && compiler.geom.stale_hits == 0`, pass `Cb { frame: None }` so `Canvas` re-blits the cached `Output` texture at `0.0%` idle CPU without recompiling or re-uploading vertex buffers.
   - `GeomCache::get` MUST track cumulative `frame_build_nanos` against the 8ms per-frame budget (`compiler.geom.budget`) and defer uncached `Text`/`Vector`/`BooleanOp` tessellation across subsequent frames via `stale_hits`.

---

## 5. DESIGN:OS & Self-Evolving Design Agents

All UI chrome, design tokens, component definitions, and critique loops in `design-os-studio` are governed by the **DESIGN:OS Self-Evolving Design System** (`design/`) in live parity with `crates/designos-app/src/theme.rs`.

### Mandatory Design Read Order (Before Any UI / Token / Canvas Change)
1. `design/principles.md` — Project design constitution, `## Always` / `## Never` laws (Purple Ban, `>=24px` pointer targets, 4px rhythm, Zero Fake UI Law).
2. `design/soul.md` — Ratified project soul (`status: ratified`) and visual/ergonomic identity.
3. `ui ds context --dir .` — Compiled design system context, token roles, and component registry.
4. `crates/designos-app/src/theme.rs` — Authoritative Rust `egui` visual constants (must remain in 100% hex parity with `design/design.tokens.json`).

### Specialized Project Design Agents (`.claude/agents/` & `.agents/agents/`)
| Agent | Role & Responsibility |
|---|---|
| **`designer-jang-designos-studio`** | Soul-bound Designer for `design:os studio`: UI generation, Rust `egui` layout/token refinement in `crates/designos-app/src/`, and component registration (`ui ds component add`). Never scores its own output; records discovered gaps via `ui memory record gap`. |
| **`curator-jang-designos-studio`** | Independent Curator & Critic (report-only; generator is NEVER grader): VSF-PCP Operate Rubric (`8×2 = 16pt`) scoring, `ui ds a11y` contrast verification, visual inspection via `designos --screenshot`, and verdict logging via `ui memory record taste_verdict`. |
| **`figma-jang-designos-studio`** | Canvas & `.fig` / `.pen` / MCP Bridge Specialist: drives `designos-mcp` (29 tools), `designos --apply-spec`, `.fig` / `.pen` round-trip, and `design-os figma`. Verifies every canvas mutation via `--inspect-json` or `--screenshot`. |
| **`librarian-jang-designos-studio`** | Self-Evolution & Knowledge Graduation Agent: runs `scripts/design_os_self_evolve.py`, collects open `gap` and `insight` events (`design-os librarian collect`), verifies `theme.rs` ↔ `design/design.tokens.json` parity, executes `harvest` + `heartbeat` + `memory compile`, and graduates recurrent insights (`seen > 1`) into `design/principles.md` and `scripts/zero_fake_ui_gate.py`. |

### Mandatory Pre-Commit / Post-UI-Change Verification Commands
```bash
export PATH="/Users/jangtrinh/.local/bin:$HOME/.cargo/bin:$PATH"

# 1. Zero Fake UI Deterministic Gate (14/14 invariants in crates/designos-app/src/)
python3 scripts/zero_fake_ui_gate.py

# 2. Automated Self-Evolution Engine & Rust <-> design:os Live Token Parity Audit (7/7 gates)
python3 scripts/design_os_self_evolve.py

# 3. Verify Self-Evolution Loop Proof (verdict=ALIVE, proof.level=APPLIED|IMPROVING, 0 findings)
design-os evolution
```
