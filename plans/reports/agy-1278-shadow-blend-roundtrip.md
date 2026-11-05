# Report: `agy-1278-shadow-blend-roundtrip` — Keep a Shadow's Blend Mode in `.pen` and SVG Round Trips (`#1278`)

## 0. What Could Not Be Verified

- Live Figma Desktop IPC (`--figma-Inspect`) was not invoked because the task runs in an isolated headless environment; instead, the shipped Figma Desktop application HTTP cache on disk (`~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/baa5bfdcb33de8da_0` and `d48a6add9e1dc400_0`) was inspected directly via `/opt/homebrew/bin/rea inspect-artifact` (`ev_e66ba07896ac4cfee8383211c9d2cb89c626808125bbbdffdd5ae65901d2bb56`) and targeted WASM function decoding (`plans/reports/agy-1278-shadow-blend-roundtrip.md:45`).
- `crates/designos-export/src/css.rs` (`crates/designos-export/src/css.rs:225-242`), `crates/designos-export/src/tailwind.rs` (`crates/designos-export/src/tailwind.rs:187-215`), and `crates/designos-export/src/codegen.rs` (`crates/designos-export/src/codegen.rs:560-568`) were not modified to emit a CSS/Tailwind shadow blend approximation note because those files are owned by concurrent PR `#1546` and neither `crates/designos-export/src/design_context.rs` (`crates/designos-export/src/design_context.rs:337`) nor `crates/designos-export/src/agent_context.rs` (`crates/designos-export/src/agent_context.rs:1`) contains a Dev Mode codegen hint list.
- `large_file_compile_benchmarks` in `crates/designos-gpu` (`crates/designos-gpu/src/lib.rs:1`) was skipped per the benchmark exclusion rule (`plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-workspace.log:504`).

---

## 1. Step 1 — Re-Anchor & Reverse Engineering (Recorded Before Editing Code)

### 1.1 Codebase Re-Anchor

1. **Shadow model & `blend` field (`crates/designos-doc/src/paint.rs`):**
   - `BlendMode` enum (`19` variants: `Normal`, `PassThrough`, `Darken`, `Multiply`, `ColorBurn`, `LinearBurn`, `Lighten`, `Screen`, `ColorDodge`, `LinearDodge`, `Overlay`, `SoftLight`, `HardLight`, `Difference`, `Exclusion`, `Hue`, `Saturation`, `Color`, `Luminosity`) is defined at `crates/designos-doc/src/paint.rs:150-171`, with `BlendMode::is_normal(&self) -> bool` at `crates/designos-doc/src/paint.rs:173-177`.
   - `Effect::DropShadow` is defined at `crates/designos-doc/src/paint.rs:304-315` with `#[serde(default, skip_serializing_if = "BlendMode::is_normal")] pub blend: BlendMode` at `crates/designos-doc/src/paint.rs:313-314`.
   - `Effect::InnerShadow` is defined at `crates/designos-doc/src/paint.rs:316-324` with `#[serde(default, skip_serializing_if = "BlendMode::is_normal")] pub blend: BlendMode` at `crates/designos-doc/src/paint.rs:322-323`.

2. **`.pen` export and import (`crates/designos-fig/src/pen/`):**
   - `parse_blend_mode(s: &str) -> BlendMode` (`crates/designos-fig/src/pen/paint.rs:13-36`) and `blend_mode_to_pen_str(b: BlendMode) -> &'static str` (`crates/designos-fig/src/pen/paint.rs:38-60`) map all `19` `BlendMode` variants to/from kebab-case strings (`"normal"`, `"pass-through"`, `"darken"`, `"multiply"`, `"color-burn"`, `"linear-burn"`, `"lighten"`, `"screen"`, `"color-dodge"`, `"linear-dodge"`, `"overlay"`, `"soft-light"`, `"hard-light"`, `"difference"`, `"exclusion"`, `"hue"`, `"saturation"`, `"color"`, `"luminosity"`), using the JSON key `"blendMode"` (used on nodes at `crates/designos-fig/src/pen/export.rs:177` and `crates/designos-fig/src/pen/node.rs:431`, and on paints at `crates/designos-fig/src/pen/paint.rs:163` and `crates/designos-fig/src/pen/paint.rs:659`).
   - On base `06e6b84a`, `export_effect(eff: &Effect) -> Option<Value>` (`crates/designos-fig/src/pen/export.rs:629-661`) ignored `blend` via `..`, and after our fix (`crates/designos-fig/src/pen/export.rs:629-671`) emits `"blendMode": blend_mode_to_pen_str(*blend)` when `!blend.is_normal()` (`crates/designos-fig/src/pen/export.rs:640-642`, `crates/designos-fig/src/pen/export.rs:654-656`).
   - On base `06e6b84a`, `parse_effects(effect_val: Option<&Value>, doc: &Document) -> Vec<Effect>` (`crates/designos-fig/src/pen/paint.rs:549-602`) hard-coded `blend: Default::default()`, and after our fix (`crates/designos-fig/src/pen/paint.rs:584-589`) reads `obj.get("blendMode").and_then(Value::as_str).map(parse_blend_mode).unwrap_or(BlendMode::Normal)` (`crates/designos-fig/src/pen/paint.rs:584`).

3. **SVG filter export (`crates/designos-export/src/svg.rs`):**
   - `blend_css(b: BlendMode) -> Option<&'static str>` is at `crates/designos-export/src/svg.rs:91-113` (used for CSS `mix-blend-mode` on groups/paints at `crates/designos-export/src/svg.rs:193` and `crates/designos-export/src/svg.rs:284`, and on noise filters at `crates/designos-export/src/svg.rs:847`).
   - `fe_blend_mode(b: BlendMode) -> &'static str` (`crates/designos-export/src/svg.rs:115-134`) maps the `16` W3C Filter Effects `<feBlend>` mode tokens (`normal`, `darken`, `multiply`, `color-burn`, `lighten`, `screen`, `color-dodge`, `overlay`, `soft-light`, `hard-light`, `difference`, `exclusion`, `hue`, `saturation`, `color`, `luminosity`) and falls back to `"normal"` for the `3` modes SVG `<feBlend>` cannot express (`BlendMode::PassThrough`, `BlendMode::LinearBurn`, `BlendMode::LinearDodge` at `crates/designos-export/src/svg.rs:117`).
   - In `shadow_filter(&mut self, n: &Node) -> String` (`crates/designos-export/src/svg.rs:725-795`), when any visible `Effect::DropShadow` has `!blend.is_normal()` (`crates/designos-export/src/svg.rs:735`), `<feFlood flood-opacity="0" result="BackgroundImageFix"/>` is emitted (`crates/designos-export/src/svg.rs:737`) and each drop shadow with `!blend.is_normal()` emits `<feBlend mode="{}" in="shadow{i}" in2="BackgroundImageFix" result="shadow{i}"/>` (`crates/designos-export/src/svg.rs:761-763`); each visible `Effect::InnerShadow` emits `<feBlend mode="{}" in="innerShadow{i}" in2="{current}" result="withInner{i}"/>` using `fe_blend_mode(*blend)` (`crates/designos-export/src/svg.rs:792`).

4. **SVG filter import (`crates/designos-svg/src/effects.rs`):**
   - `blend_of(b: usvg::BlendMode) -> BlendMode` (`crates/designos-svg/src/effects.rs:15-35`) maps all `16` `usvg::BlendMode` variants to `designos_doc::BlendMode` (matching `crates/designos-svg/src/convert.rs:78-98`).
   - `effects_of(filters: &[std::sync::Arc<usvg::filter::Filter>]) -> Vec<Effect>` (`crates/designos-svg/src/effects.rs:37-43`) calls `one_filter(f.primitives())` (`crates/designos-svg/src/effects.rs:56-156`).
   - In `one_filter`, when `Kind::ColorMatrix(m)` (`crates/designos-svg/src/effects.rs:100-133`) pushes an `Effect::InnerShadow` (`crates/designos-svg/src/effects.rs:110-118`) or `Effect::DropShadow` (`crates/designos-svg/src/effects.rs:120-128`), it records `(p.result().to_string(), idx, false)` in `shadow_results` (`crates/designos-svg/src/effects.rs:78`, `crates/designos-svg/src/effects.rs:130`). When `Kind::Blend(b)` (`crates/designos-svg/src/effects.rs:134-150`) is encountered, it resolves the target shadow via `b.input1()` (`crates/designos-svg/src/effects.rs:135-138`) or `b.input2()` (`crates/designos-svg/src/effects.rs:139-142`) and writes `*blend = blend_of(b.mode())` (`crates/designos-svg/src/effects.rs:146`).

5. **CPU and GPU shadow blend rendering (`crates/designos-render` and `crates/designos-gpu`):**
   - CPU renderer (`crates/designos-render/src/lib.rs:612-644`) passes `*blend` from `Effect::DropShadow` (`crates/designos-render/src/lib.rs:612-624`) to `shadow::drop_shadow_rrect` (`crates/designos-render/src/shadow.rs:254-288`) and `effects::drop_shadow` (`crates/designos-render/src/effects.rs:92-151`), and from `Effect::InnerShadow` (`crates/designos-render/src/lib.rs:633-644`) to `effects::inner_shadow` (`crates/designos-render/src/effects.rs:154-185`). Both composite via `composite_blended(target, rx, ry, &pm, blend, clip)` (`crates/designos-render/src/effects.rs:74-82`, called at `crates/designos-render/src/effects.rs:150`, `crates/designos-render/src/effects.rs:184`, and `crates/designos-render/src/shadow.rs:286`).
   - GPU compiler (`crates/designos-gpu/src/compile.rs:1116-1148`, `crates/designos-gpu/src/compile.rs:1469-1550`) checks `special_blend(blend)` (`!matches!(blend, BlendMode::Normal | BlendMode::PassThrough)` at `crates/designos-gpu/src/compile.rs:1116-1118`) and maps `BlendMode` via `shadow_mode(blend)` (`crates/designos-gpu/src/compile.rs:1120-1148`), bypassing analytic drop-shadow fast-paths when `special_blend(blend)` is true (`crates/designos-gpu/src/compile.rs:1469`) and emitting `Op::Composite` with `mode: shadow_mode(blend)` for both `drop_shadow` (`crates/designos-gpu/src/compile.rs:1497`) and `inner_shadow` (`crates/designos-gpu/src/compile.rs:1549`).

6. **Dev Mode (`crates/designos-export/src/design_context.rs` & `crates/designos-export/src/agent_context.rs`):**
   - `crates/designos-export/src/design_context.rs:337` and `crates/designos-export/src/design_context.rs:403` only count `Effect::DropShadow` / `Effect::InnerShadow` for visual annotation badges and style summaries; `crates/designos-export/src/agent_context.rs:1-120` generates MCP agent markdown. Neither file has a Dev Mode codegen hint list. A CSS/Tailwind shadow blend approximation comment would belong in `crates/designos-export/src/css.rs:225-242`, `crates/designos-export/src/tailwind.rs:187-215`, and `crates/designos-export/src/codegen.rs:560-568` (left untouched per Ruling 4 because those files are owned by concurrent PR `#1546`).

### 1.2 Figma Reverse Engineering (`rea`)

- **Artifacts inspected:**
  1. `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/baa5bfdcb33de8da_0` — Brotli-compressed `https://www.figma.com/webpack-artifacts/assets/figma_app-9b837f4ccec188a9.min.js.br` (decoded `8,396,757` bytes, SHA-256 `48b375393b89dd446cd0440b56c777cc12af668ae3e8811f321b3f18291c5a23`, REA `inspect-artifact` evidence ID `ev_e66ba07896ac4cfee8383211c9d2cb89c626808125bbbdffdd5ae65901d2bb56`).
     - At byte offsets `6658102` and `6658498` (`figma_app-9b837f4ccec188a9.min.js:6658102`), Figma's bundled SVG filter templates emit:
       `<feFlood flood-opacity="0" result="BackgroundImageFix"/><feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha"/>...<feBlend mode="normal" in2="BackgroundImageFix" result="effect1_dropShadow_276_2240"/>...<feBlend mode="normal" in="SourceGraphic" in2="effect2_dropShadow_276_2240" result="shape"/>`.
  2. `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/d48a6add9e1dc400_0` — Brotli-compressed `https://www.figma.com/webpack-artifacts/assets/compiled_wasm.wasm.br` (Simple Cache payload at byte offset `142`, decoded `53,282,791` bytes).
     - `func[32177]` (`compiled_wasm.wasm:0x1ef7452..0x1efd913`) generates SVG `<filter>` primitives for `DROP_SHADOW` (`compiled_wasm.wasm:0x1ef8470`, format string at `compiled_wasm.wasm:49302533` / `vaddr=0xe1b88`) and `INNER_SHADOW` (`compiled_wasm.wasm:0x1ef933c`, format string at `compiled_wasm.wasm:49302688` / `vaddr=0xe1c23`).
     - Both shadow branches call `func[10906]` (`compiled_wasm.wasm:0x871e49..0x87248c`, invoked at `compiled_wasm.wasm:0x1ef8456` for drop shadow and `compiled_wasm.wasm:0x1ef9322` for inner shadow), which formats `<feBlend mode="%s"%s%s result="%s"/>\n` (`compiled_wasm.wasm:49906635`, `vaddr=0x17534e`) using the `19`-entry blend-mode string table at `compiled_wasm.wasm:50201936..50202012` (`vaddr=0x1be0c0`, strings at `compiled_wasm.wasm:50201708..50201936`) and overwrites `blendMode == 0` (`pass-through`) with `"normal"` (`compiled_wasm.wasm:0x871ea6..0x871ef8`, string `"normal"` at `compiled_wasm.wasm:49439770`).
     - In `figma_app-9b837f4ccec188a9.min.js:271190`, `LINEAR_BURN` and `LINEAR_DODGE` carry comment `"not supported in SVG"` and fall back to `"normal"` in SVG `<feBlend>` mode attributes.

---

## 2. Rulings & Skills Applied

- **Skills Loaded:** `es:astra-work`, `es:fable-thinking`, `es:rust`, `es:rust-gpu-canvas`, `es:dev`, `es:feature-dev`.
- **Ruling 1 (`.pen` round-trip):** `export_effect` (`crates/designos-fig/src/pen/export.rs:629-671`) writes `"blendMode": blend_mode_to_pen_str(*blend)` on `Effect::DropShadow` (`crates/designos-fig/src/pen/export.rs:640-642`) and `Effect::InnerShadow` (`crates/designos-fig/src/pen/export.rs:654-656`) when `!blend.is_normal()`, omitting `"blendMode"` when `blend == BlendMode::Normal` so `Normal` output is byte-identical. `parse_effects` (`crates/designos-fig/src/pen/paint.rs:584-589`) parses `"blendMode"` via `parse_blend_mode` (`crates/designos-fig/src/pen/paint.rs:13-36`) and defaults missing values to `BlendMode::Normal` (`crates/designos-fig/src/pen/paint.rs:584`).
- **Ruling 2 (SVG filter export):** `fe_blend_mode` (`crates/designos-export/src/svg.rs:115-134`) maps the `16` SVG `<feBlend>` modes and falls back to `"normal"` for `PassThrough`, `LinearBurn`, and `LinearDodge` (`crates/designos-export/src/svg.rs:117`). `shadow_filter` (`crates/designos-export/src/svg.rs:725-795`) emits `<feFlood flood-opacity="0" result="BackgroundImageFix"/>` (`crates/designos-export/src/svg.rs:737`) and `<feBlend mode="{}" in="shadow{i}" in2="BackgroundImageFix" result="shadow{i}"/>` (`crates/designos-export/src/svg.rs:761-763`) when a drop shadow has `!blend.is_normal()`, and emits `<feBlend mode="{}" in="innerShadow{i}" in2="{current}" result="withInner{i}"/>` (`crates/designos-export/src/svg.rs:792`) for inner shadows.
- **Ruling 3 (SVG filter import):** `one_filter` (`crates/designos-svg/src/effects.rs:56-156`) tracks each shadow primitive's `result` name in `shadow_results` (`crates/designos-svg/src/effects.rs:78`, `crates/designos-svg/src/effects.rs:130`) and maps `Kind::Blend(b)` back onto the corresponding shadow's `blend` field via `blend_of(b.mode())` (`crates/designos-svg/src/effects.rs:15-35`, `crates/designos-svg/src/effects.rs:134-150`).
- **Ruling 4 (Dev Mode):** Documented in Section `0` and Section `1.1(6)` above (`crates/designos-export/src/design_context.rs:337`, `crates/designos-export/src/agent_context.rs:1`).
- **Ruling 5 (Byte-for-byte / pixel-for-pixel preservation for `Normal`):** Verified by `normal_shadow_blend_omits_blend_mode_in_pen_export_and_defaults_on_import` (`crates/designos-fig/src/pen/export.rs:843-884`) and `normal_shadow_filter_output_is_byte_identical_to_base` (`crates/designos-export/src/svg.rs:1528-1555`).

---

## 3. Acceptance Table & Verification Evidence

| # | Criterion | Status | Citation |
|---|---|---|---|
| **1(a)** | Drop shadow and inner shadow set to `Multiply` survive `.pen` export then import as `Multiply` (RED on `06e6b84a`, GREEN on HEAD) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:16-69`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:15-17`, `crates/designos-fig/src/pen/export.rs:808-840` |
| **1(b)** | SVG export of `Multiply` and `Screen` drop shadow and inner shadow writes `feBlend mode="multiply"` / `feBlend mode="screen"` at the shadow's composite step (RED on `06e6b84a`, GREEN on HEAD) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:136-174`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:50-52`, `crates/designos-export/src/svg.rs:1447-1484` |
| **1(c)** | SVG export then SVG import of (b) gives `Multiply` (and `Screen`) shadows back (RED on `06e6b84a`, GREEN on HEAD) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:190-255`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:63-80`, `crates/designos-svg/src/effects.rs:163-199`, `crates/designos-svg/tests/shadow_blend_roundtrip.rs:73-98` |
| **1(d)** | Modes SVG cannot express (`LinearBurn`, `LinearDodge`, `PassThrough`) fall back to `normal` (RED on `06e6b84a`, GREEN on HEAD) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:105-134`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:257-295`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:50-52`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:78-80`, `crates/designos-export/src/svg.rs:1487-1525`, `crates/designos-svg/tests/shadow_blend_roundtrip.rs:101-120` |
| **1(e)** | CPU render of SVG-reimported node with `Multiply` shadows matches original node's CPU render within project tolerance `0.02` (`mismatch=0.2217` on `06e6b84a` -> `< 0.02` on HEAD) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:297-309`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:78-80`, `crates/designos-svg/tests/shadow_blend_roundtrip.rs:123-135` |
| **1(null)** | Null control: `Normal`-blend shadows give byte-identical `.pen` and SVG output to base `06e6b84a` | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:11-12`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:96`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:12`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:33`, `crates/designos-fig/src/pen/export.rs:843-884`, `crates/designos-export/src/svg.rs:1528-1555` |
| **2(m1)** | Mutant 1 killed: SVG export reverted to hard-coded `normal` fails with `mutant1_export_rc=101` and `mutant1_svg_rt_rc=101` | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/mutant-1-svg-hardcoded-normal.log:25-64`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/mutant-1-svg-hardcoded-normal.log:91-144` |
| **2(m2)** | Mutant 2 killed: `.pen` import ignoring `"blendMode"` fails with `mutant2_fig_rc=101` (`left: Normal, right: Multiply`) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/mutant-2-pen-import-ignores-field.log:16-29` |
| **3(g1)** | Gate: `cargo fmt --all -- --check` (`fmt_rc=0`) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1-2` |
| **3(g2)** | Gate: workspace clippy `--all-targets --locked` (`clippy_workspace_rc=0`, `clippy_strict_touched_rc=0`) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-workspace.log:426`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-workspace.log:503` |
| **3(g3)** | Gate: `cargo test -p designos-fig --locked` (`test_fig_rc=0`, `145` lib tests + integration tests passed) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:158`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:840` |
| **3(g4)** | Gate: `cargo test -p designos-export --locked` (`test_export_rc=0`, `168` lib tests + integration tests passed) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1022`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1315` |
| **3(g5)** | Gate: `cargo test -p designos-svg --locked` (`test_svg_rc=0`) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1394` |
| **3(g6)** | Gate: `cargo test -p designos-render --locked` (`test_render_rc=0`, `56` lib tests + integration tests passed) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1464`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1875` |
| **3(g7)** | Gate: `cargo test --workspace --locked --lib -- --skip large_file_compile_benchmarks` (`workspace_lib_rc=0`) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-workspace.log:2396` |
| **3(g8)** | Gate: `python3 -B scripts/zero_fake_ui_gate.py` (`17/17` invariants, `zero_fake_ui_rc=0`) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1900-1902` |
| **Cleanup** | `rm -rf out/cargo` and `ls out/cargo` failing (`ls_out_cargo_rc=1`) | **PASS** | `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/cleanup-out-cargo.log:1-2` |

### 3.1 Pasted RED Run on Base `06e6b84a` (`plans/reports/agy-1278-shadow-blend-roundtrip-evidence/red-base-06e6b84a.log:1-309`)

```text
=== RED RUN ON BASE 06e6b84a ===
running 2 tests
test pen::export::tests::shadow_blend_mode_survives_pen_roundtrip ... FAILED
test pen::export::tests::normal_shadow_blend_omits_blend_mode_in_pen_export_and_defaults_on_import ... ok

thread 'pen::export::tests::shadow_blend_mode_survives_pen_roundtrip' (85197061) panicked at crates/designos-fig/src/pen/export.rs:803:9:
expected exported .pen JSON to contain blendMode multiply
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 143 filtered out; finished in 0.02s
fig_red_rc=101

running 22 tests
test svg::tests::normal_shadow_filter_output_is_byte_identical_to_base ... ok
test svg::tests::shadow_filter_unsupported_svg_blend_modes_fall_back_to_normal ... FAILED
test svg::tests::shadow_filter_exports_multiply_and_screen_blend_modes ... FAILED
test result: FAILED. 20 passed; 2 failed; 0 ignored; 0 measured; 146 filtered out; finished in 0.10s
export_red_rc=101

running 1 test
test effects::tests::parses_blend_mode_from_drop_and_inner_shadow_filter_chains ... FAILED
thread 'effects::tests::parses_blend_mode_from_drop_and_inner_shadow_filter_chains' (85198822) panicked at crates/designos-svg/src/effects.rs:154:9:
got DropShadow { color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 }, offset: Vec2 { x: 0.0, y: 4.0 }, radius: 8.0, spread: 0.0, visible: true, show_behind: false, blend: Normal }
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
svg_lib_red_rc=101

running 3 tests
test svg_roundtrip_preserves_multiply_and_screen_shadow_blend_modes ... FAILED
test svg_roundtrip_unsupported_shadow_blend_modes_fall_back_to_normal ... FAILED
test cpu_render_of_svg_reimported_multiply_shadow_node_matches_original ... FAILED
thread 'cpu_render_of_svg_reimported_multiply_shadow_node_matches_original' (85199274) panicked at crates/designos-svg/tests/shadow_blend_roundtrip.rs:131:5:
expected SVG-reimported Multiply shadow CPU render to match original within 0.02, got mismatch=0.2217
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
svg_rt_red_rc=101
```

### 3.2 Pasted GREEN Acceptance Run (`plans/reports/agy-1278-shadow-blend-roundtrip-evidence/green-acceptance.log:1-80`)

```text
=== GREEN ACCEPTANCE RUN ===
running 2 tests
test pen::export::tests::normal_shadow_blend_omits_blend_mode_in_pen_export_and_defaults_on_import ... ok
test pen::export::tests::shadow_blend_mode_survives_pen_roundtrip ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 143 filtered out; finished in 0.01s
fig_green_rc=0

running 22 tests
test svg::tests::normal_shadow_filter_output_is_byte_identical_to_base ... ok
test svg::tests::shadow_filter_exports_multiply_and_screen_blend_modes ... ok
test svg::tests::shadow_filter_unsupported_svg_blend_modes_fall_back_to_normal ... ok
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 146 filtered out; finished in 0.08s
export_green_rc=0

running 1 test
test effects::tests::parses_blend_mode_from_drop_and_inner_shadow_filter_chains ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
svg_lib_green_rc=0

running 3 tests
test svg_roundtrip_unsupported_shadow_blend_modes_fall_back_to_normal ... ok
test svg_roundtrip_preserves_multiply_and_screen_shadow_blend_modes ... ok
test cpu_render_of_svg_reimported_multiply_shadow_node_matches_original ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
svg_rt_green_rc=0
```

### 3.3 Pasted Mutation Runs (`plans/reports/agy-1278-shadow-blend-roundtrip-evidence/mutant-1-svg-hardcoded-normal.log:1-144`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/mutant-2-pen-import-ignores-field.log:1-29`)

```text
=== MUTANT 1: SVG EXPORT HARD-CODED NORMAL ===
running 1 test
test svg::tests::shadow_filter_exports_multiply_and_screen_blend_modes ... FAILED
thread 'svg::tests::shadow_filter_exports_multiply_and_screen_blend_modes' (85232107) panicked at crates/designos-export/src/svg.rs:1475:13:
expected drop shadow feBlend mode=multiply
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 167 filtered out; finished in 0.00s
mutant1_export_rc=101

running 3 tests
test svg_roundtrip_unsupported_shadow_blend_modes_fall_back_to_normal ... ok
test svg_roundtrip_preserves_multiply_and_screen_shadow_blend_modes ... FAILED
test cpu_render_of_svg_reimported_multiply_shadow_node_matches_original ... FAILED
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
mutant1_svg_rt_rc=101

=== MUTANT 2: PEN IMPORT IGNORES BLEND_MODE ===
running 1 test
test pen::export::tests::shadow_blend_mode_survives_pen_roundtrip ... FAILED
thread 'pen::export::tests::shadow_blend_mode_survives_pen_roundtrip' (85238445) panicked at crates/designos-fig/src/pen/export.rs:825:17:
assertion `left == right` failed
  left: Normal
 right: Multiply
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 144 filtered out; finished in 0.01s
mutant2_fig_rc=101
```

### 3.4 Pasted Gate & Cleanup Outputs (`plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-crates.log:1-1902`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/gates-workspace.log:1-2396`, `plans/reports/agy-1278-shadow-blend-roundtrip-evidence/cleanup-out-cargo.log:1-2`)

```text
=== GATE 1: CARGO FMT CHECK ===
fmt_rc=0
=== GATE 3: CARGO TEST DESIGNOS-FIG ===
test_fig_rc=0
=== GATE 4: CARGO TEST DESIGNOS-EXPORT ===
test_export_rc=0
=== GATE 5: CARGO TEST DESIGNOS-SVG ===
test_svg_rc=0
=== GATE 6: CARGO TEST DESIGNOS-RENDER ===
test_render_rc=0
=== GATE 8: ZERO FAKE UI GATE ===
 Gate Verdict: PASS (17/17 invariants satisfied)
zero_fake_ui_rc=0

=== GATE 2: WORKSPACE CLIPPY ===
clippy_workspace_rc=0
=== GATE 2B: STRICT CLIPPY ON TOUCHED CRATES ===
clippy_strict_touched_rc=0
=== GATE 7: WORKSPACE LIB TESTS ===
workspace_lib_rc=0

ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

### 3.5 Pasted Git & Draft PR State (`plans/reports/agy-1278-shadow-blend-roundtrip-evidence/git-state.log:1-4`)

- **Draft PR:** `https://github.com/jangtrinh/design-os-studio/pull/1577` (`#1577` against `main`, closes `#1278`, part of `#18`).

```text
=== GIT STATUS -S ===
(clean working tree)
=== GIT LOG --ONELINE ORIGIN/MAIN..HEAD ===
a5d66a29 fix(effects): keep a shadow's blend mode in .pen and SVG round trips (#1278)
```

---

## 4. Assumptions

- `ASSUMPTION`: External SVG producers that emit shadow filter chains either set `in` on `<feBlend>` to the shadow's `<feColorMatrix>` `result` (or omit `in`, which `usvg` resolves to the preceding primitive's `result` at `usvg-0.45.1/src/parser/filter.rs:471-475`), or set `in="SourceGraphic"` and `in2` to the shadow's `<feColorMatrix>` `result`. Both forms are handled in `crates/designos-svg/src/effects.rs:134-150`.
- `ASSUMPTION`: Existing `.pen` files created before shadow `"blendMode"` was exported omit `"blendMode"`, which `parse_effects` (`crates/designos-fig/src/pen/paint.rs:584`) defaults to `BlendMode::Normal`.

---

## 5. Weakest Link

- In SVG 1.1 / W3C Filter Effects (`crates/designos-export/src/svg.rs:735-763`), `BackgroundImage` is not supported by browsers or `resvg` (`usvg-0.45.1/src/parser/filter.rs:488-490`), so an isolated SVG `<filter>` blends a drop shadow against transparent `BackgroundImageFix` (`<feFlood flood-opacity="0" result="BackgroundImageFix"/>`). While the `<feBlend mode="...">` attribute round-trips losslessly through `designos-svg` (`crates/designos-svg/src/effects.rs:134-150`) and renders with full backdrop blending in Studio's CPU renderer (`crates/designos-render/src/lib.rs:612-624`) and GPU renderer (`crates/designos-gpu/src/compile.rs:1469-1497`), standalone SVG viewers (`resvg`) cannot blend an isolated `<filter>` primitive against the canvas backdrop outside the filter's `SourceGraphic`.

Status: READY_FOR_PR
