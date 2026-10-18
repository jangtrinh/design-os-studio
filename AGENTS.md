# DESIGN:OS Studio (`design-os-studio`) — Cross-Runtime Agent Instructions (`AGENTS.md`)

This repository is **DESIGN:OS Studio**, the native Rust + `wgpu` (Metal/Vulkan/GL) GPU-accelerated design canvas, `.fig` / `.designos` / `.designos` editor, and headless MCP server (`designos-mcp` / `designos-mcp`) for the **DESIGN:OS / EaseUI** ecosystem.

All agents (Claude Code, Codex, Antigravity, Grok, Orca workers) operating in this repository are bound by the same architectural contracts as [`CLAUDE.md`](file:///Users/jangtrinh/Products/design-os-studio/CLAUDE.md), [`.project-agent.md`](file:///Users/jangtrinh/Products/design-os-studio/.project-agent.md), and [`CONTEXT.md`](file:///Users/jangtrinh/Products/design-os-studio/CONTEXT.md).

---

## 1. Mandatory Skills & Context Loading

1. **Required Skills:**
   - `es:rust` — Idiomatic Rust 2021, zero-clone data flow, lock-order safety, and memory efficiency on 700k+ node graphs.
   - `es:rust-gpu-canvas` — `wgpu` pipeline architecture, `egui-wgpu` render pass integration, `lyon` tessellation budgeting, `SceneIndex` spatial culling, and CPU/GPU reference parity.
   - `es:astra-work` — Evidence-backed execution loop and post-mutation verification.
   - `es:fable-thinking` — Epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), falsification-first root-cause analysis, and prediction before action.
2. **Project Knowledge Base (`.brv/`):**
   - Read `.brv/context-tree/session-handoffs/latest.md` before starting feature or bugfix work.
   - Read `.brv/context-tree/architecture/crate-map.md` and `.brv/context-tree/architecture/figma-vs-designos-engine.md` before modifying document serialization, `.fig` import, `SceneIndex`, or GPU compilation.

---

## 2. Workspace Crate Map (11 Crates)

| Crate | Directory | Summary |
|---|---|---|
| **`designos-doc`** | `crates/designos-doc` | Scene graph (`Document`, `Node`, `NodeId`, `NodeKind`), compact single-line JSON + `"hex:..."` image serialization, styles, variables, components, prototyping, undo/redo. |
| **`designos-fig`** | `crates/designos-fig` | Figma `.fig` (`fig-kiwi` binary container) and REST JSON importer/exporter, VectorNetwork conversion, component/instance expansion. |
| **`designos-render`** | `crates/designos-render` | Deterministic CPU reference renderer (`tiny-skia`), `SceneIndex` spatial/paint-order index, `TileCache`, `i_overlay` boolean ops, `cosmic-text` layout. |
| **`designos-gpu`** | `crates/designos-gpu` | `wgpu` GPU renderer (`SceneCompiler`, `GeomCache`, `PaintCache`, `Renderer`, WGSL shaders) with `lyon` tessellation and 8ms frame budget. |
| **`designos-app`** | `crates/designos-app` | `egui`/`eframe` desktop app (`designos-studio` / `designos`), EaseUI BrainWave 2.0 dark/light theme, GPU/tile canvas, virtualized Layers panel, Home library, CLI. |
| **`designos-library`** | `crates/designos-library` | On-disk project store (`Library`), 8 KB `file_kind` header probe, single-pass `import_file_with_doc`, thumbnails, trash, version history. |
| **`designos-fonts`** | `crates/designos-fonts` | Bundled Inter + Fontsource on-demand font fetcher, disk cache, and zero-clone `ensure_font_requests_with`. |
| **`designos-svg`** | `crates/designos-svg` | SVG import (`usvg` -> `designos_doc::Node` tree). |
| **`designos-export`** | `crates/designos-export` | PNG/JPG/SVG/PDF export and Dev Mode code generation (`CSS`, `Tailwind`, `React`, `SwiftUI`, `Compose`). |
| **`designos-mcp`** | `crates/designos-mcp` | Stdio JSON-RPC MCP server (`designos-mcp` / `designos-mcp`) for AI agent canvas/document automation. |
| **`designos-cli` / `designos-diff`** | `crates/designos-diff` + `crates/designos-app/src/cli.rs` | Pixel-diff verification CLI (`designos-diff`) and desktop CLI automation interface (`--open-id`, `--screenshot`, `--gpu`, `--fixture`). |

---

## 3. Build, Package & Benchmark Commands

```bash
export PATH="$HOME/.cargo/bin:$PATH"

# 1. Build release desktop app & MCP server
cargo build --release -p designos-app -p designos-mcp
cargo build --release -p designos-app

# 2. Update macOS app bundle at target/release/bundle/DESIGN-OS Studio.app
cp target/release/designos-studio "target/release/bundle/DESIGN-OS Studio.app/Contents/MacOS/designos-studio"
cp target/release/designos "target/release/bundle/DESIGN-OS Studio.app/Contents/MacOS/designos"

# 3. Open the "VSF - PCP.fig" enterprise benchmark directly
# (1,058,895 Kiwi records -> 336,214 initial / 1,082,324 full scene nodes across 31 pages;
#  112,474 scene nodes / 723,136 Kiwi records on the 7-page subset)
"target/release/bundle/DESIGN-OS Studio.app/Contents/MacOS/designos-studio" "$HOME/Downloads/VSF - PCP.fig"

# 4. Run test gates
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
- Kit content keeps the kit's original colours, purple included (owner: "Purple này là style của Untitled nên chấp nhận", i.e. this purple is Untitled's own style, so accept it). The Purple Ban still applies to the film's own chrome and type, not to kit content. Never recolour pixels. Photos of people in kit content are allowed (owner: "Show people cũng được, kệ đi").
- Demos must be true to the released build. Pick kit frames that the release renders the way Figma does: the CPU and `--gpu` renders match, and no known render bug is hit. Never show behaviour the code does not have. For example, agent undo is one step per tool call, and a paste from Figma drops styles.
- Credit "Untitled UI PRO v8.0 kit by Untitled UI" in the end card.

## 4. Non-Negotiable Performance & Safety Invariants

1. **No `doc.to_json()` in `doc_hash` (`crates/designos-app/src/home/session.rs`):**
   - Always hash documents via the streaming, zero-allocation `HashSerializer`. Never serialize `Document` to a `String` for change detection or autosave probing, and never bump `TileView::rev` (`app.changed()`) on viewport-only pan/zoom/fit gestures.
2. **No `ctx.load_texture` inside `ctx.data_mut` (`crates/designos-app/src/icons.rs`):**
   - `egui::Context::data_mut` holds a non-reentrant `parking_lot::RwLock` write guard. Calling `ctx.load_texture` inside `ctx.data_mut` deadlocks the main thread on Frame 1. Decode and call `ctx.load_texture` outside `ctx.data_mut`.
3. **Preserve `interactive()` Guard in `designos-gpu` (`crates/designos-gpu/src/compile.rs`):**
   - `self.interactive()` (`self.cull && self.geom.budget.is_some()`) separates live interactive rendering from headless test rendering. Keep all LOD approximations (`< 0.75px` culling, `< 3.5px` text greeking, scissor clip fallback, `< 64` `Program` cap) strictly inside `self.interactive()` so headless reference tests (`cargo test -p designos-gpu`) remain bit-exact against `designos-render`.
4. **Cap Offscreen GPU `Program`s & Preserve `FrameKey` Reuse:**
   - Do not allocate new offscreen `Program`s beyond `self.progs.len() < 64` during interactive frames, and preserve `FrameKey` caching (`Cb { frame: None }` when `last_key == Some(key) && stale_hits == 0`) plus `frame_build_nanos` 8ms cumulative tessellation budgeting so idle CPU stays at `0.00%` and `egui-wgpu`'s Metal staging buffer never overflows.
   - `Autosave::autosave_tick` (`crates/designos-app/src/home/session.rs`) watches `watched_disk_path()` off the UI thread (`DiskWatcher`) and calls `ctx.request_repaint()` only when `modified()` changes (no periodic UI-thread repaint timer); `canvas::grids::paint` takes its frames from `SceneIndex::grid_nodes()` (drawn frames only, kept in paint order) and walks `page.children` only when no scene index is available. Measured idle CPU on Apple M4 Max (`ps -p <pid> -o time=` cumulative CPU delta on `target/release/designos "VSF - PCP (Copy).fig" --gpu --select billing` after initial load + 250 ms `IDLE_REFINE_DELAY`): `0.00% / 0.00%` (`dCPU = 0.000s` across two 20 s idle windows / 40 s total).

<!-- BEGIN ease-design -->
## ease-design

This project uses ease-design. Workflows, skills, and journeys live under
`/opt/homebrew/lib/node_modules/ease-design/templates/workflows/`, `/opt/homebrew/lib/node_modules/ease-design/templates/skills/`, and `/opt/homebrew/lib/node_modules/ease-design/templates/journeys/`.
Invoke them by following the relevant Markdown file when the user asks for
design work (journeys cover onboarding/daily/delivery sequencing across
multiple commands). Templates reference `knowledge/<file>` — resolve those against `/opt/homebrew/lib/node_modules/ease-design/knowledge`. The `ui` binary handles all non-LLM
work (autofix, layout validation, token compilation, color math). Before
forming a `ui` invocation, run `ui schema --json` for the machine-readable
signature (positionals, flags, enums, error codes) of every (sub)command.

Available slash-commands when proxied:
/ui:generate /ui:native-macos /ui:native-ios /ui:native-ipados /ui:iterate /ui:refine /ui:redesign /ui:extract /ui:learn /ui:why /ui:from-ref /ui:from-url /ui:figma /ui:figma-comments /ui:to-figma /ui:audit /ui:design /ui:diagram /ui:chart /ui:slides /ui:evidence /ui:init.

Template hashes (sha256, for drift detection):
  journeys/daily.md: 8ff71a16d707fad4c79e20efc9d32ca1e3bdd2b66ed65f34434933a2482a2834
  journeys/deliver.md: 87cc8f96115a8f6a4cfd3ac27fb920fd2e37e2f7d494a30ecf8ccf79528e6f2c
  journeys/onboard.md: 1fe82ccec8e21a29fc9d00b23dfa8ad7bdab4ce912564d074d258d52dfeec67f
  skills/apply-prompt-mode.md: fd999d84cb87170b8267f0e4c2c7577faddb96791afea77c5fd4af5263336e94
  skills/canvas-effect.md: d9f6e8e1a54e26ac27cf97c0d77fbabb740013a0dd47dc7b8a8127e9be5f302b
  skills/chart-craft.md: 59a9be0faa2cda77f039cb11728ba1581352986658cd3502c11917f42f604cfa
  skills/check-consistency.md: 9ab193d1c78eda2024a8216e3377b59b7c63c6c443f160ea8bbaa28121af503b
  skills/color-decision.md: 99919c64baf7c32c0a075df688d5f6e8fff337b3b9b14826df8a341b0beaf526
  skills/designmd-emit.md: 6bc6bba68ed7aa4859ef7f57b7c1b1ee37749be1f1bc5129f52ca9ca05cbce81
  skills/diagram-craft.md: 2c3c8447da17f79c941b436c859952307a59aac9b7d76835ebc18f1b5edee9a2
  skills/figma-craft.md: aec81304427e44001a48588b335deed53eaa156e22a5e09705055738f98c00e8
  skills/gflow-flight.md: 10a0cad32057a4ef5f7d2621f17c015377fe86a22a2e70ae50170229867d578d
  skills/gsap-motion.md: dfd7cdcc42795c3412f0680914ca187bdbe3d83b38f19c9e78740d9ff4c4effd
  skills/native-ios-craft.md: 596138634ba0ab887fb163456daf28379d502de080641ccd6552cba2506b2bad
  skills/native-ipados-craft.md: 5075d18cbb257524731effcf11f7c15f39bc695f8c656faf3b4d6fa848aff83b
  skills/native-macos-craft.md: 3154abdb10f643bdfbdbe595182ae3a2243ddd39db7c559487b8deba14c67aa4
  skills/pick-persona.md: 7d4b1509898767bd1ba811bd2268e323f82cdde43674ae89ece021ba34e6f5a6
  skills/score-taste.md: d78f9476e7499559af0fe793a59af94a9e426e4c65dbda435e0618e40bfbd313
  skills/scroll-cinema.md: efad7e3dd56570cf301ed389b7f092abd07ca0b21a559b722a554991751de733
  skills/shader-gradient.md: 533300fa259f642c940f6a1280e47006ed357955608631697971846b046ca11d
  skills/token-model.md: 20e21df678deaf59a8fc96eea0d4eadad1d414e85eec066ee2c3869d5dc4a0e6
  skills/verify-canvas.md: 950bd2b21fcb250a5d382f70c7e394be12a7e575d8bb145a3ff1d98938f11400
  workflows/audit.md: 8ee10c9a645b7c9491e85e5584e847c47f3d6b8cc35039aec6e23c81ba779d7c
  workflows/chart.md: 04e6ef0b1df3e186bdaedb6793f092bad01c35a633e160d4d705123e26d91be5
  workflows/design.md: bfafc9ef13bd4b1108225778c05521db790ea798c385767a56a278d2bf67ebe2
  workflows/diagram.md: 424989eaf3548ea67048a5b3bd265439495a53d9dd3a3808e1d3a51071f76086
  workflows/evidence.md: b11b033bc022945335f664aeabe7da4e6f35286a788deac20b4bb9c499e7fdb8
  workflows/extract.md: cac2216ffec9b86da9ee3f27a9fd4f262c7787c92787418b6960224dfc734669
  workflows/figma-comments.md: de8102f574898a9a89854bef12f84c9c03c4ac60b0d2cce2c2860809ba64d009
  workflows/figma.md: 1c528b97ca34489f27ddea641730e950273209d8314a879566e6b3e5b93a9d36
  workflows/from-ref.md: dd7741c453f4b3a039271c90bef1dafb6aaa500f2792d5bcc478b2f0eefa259a
  workflows/from-url.md: 8b40ae5d9781ca59094ef9aefa5977d0f942fa4af1f3463e287a08ee61e72167
  workflows/generate.md: df9978fd40b9e605522a4e71b2b62a958fe5895820ca32e357de47a815e93d16
  workflows/iterate.md: 1adceeb5d599f55897164e9a0bd387b5213fcb5669d70258409736a78c245af0
  workflows/learn.md: 30f1f4dcc663550b7228f87178031f4c4d0f3d2b87f6bcfd8467afe3d9628f53
  workflows/native-ios.md: 5c7ff1727ba8b7e070a44bb33ef24614b55853acf947c4bf03cf8876bc902078
  workflows/native-ipados.md: 39a5df50990aca2d6a67b6d6e6bab114c87d987b6380bbeb5298d0132b931828
  workflows/native-macos.md: e0328b4aaa23f00bdbd8b05ba0768313c122ff72a2b22933f3f70203ca5f8bd7
  workflows/redesign.md: 6c69091011bca95923f5f41c11361dcebc2ada5ee73397fbf9d02c79029592cc
  workflows/refine.md: ec50c0a8213a1823883ce16451c274377474cdf6d732eb6b14194ee86b05df13
  workflows/slides.md: 208cb15bbe82af7902327fc41c6e5d2b1e8a74e0aef44a20542ed502e08409f0
  workflows/to-figma.md: d00b6e45e0c057f94e6dead06d70f78b6356eb1d0bb952bfec403460b1b3fff5
  workflows/why.md: 13b46daa75e15e60438da4c640410a3d8851ef83416a04098a7e5ba1759a67a3

Do not edit content between the BEGIN/END markers — it is regenerated
by `ui init --runtime codex --force`.
<!-- END ease-design -->
