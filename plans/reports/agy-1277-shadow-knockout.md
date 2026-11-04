# Issue #1277 — Fill-less frame shadow knockout follows what the children paint

## What Could Not Be Verified

- Live Figma Desktop MCP (`use_figma`) comparison on a running Figma Desktop canvas was not used because no Figma Desktop bridge session is attached in this headless worker environment; instead, Figma's exact RenderTree and shader knockout logic was verified statically via `rea` on `/Applications/Figma.app/Contents/MacOS/Figma` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:2`) and the desktop HTTP cache `compiled_wasm.wasm` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:6`), and all five rulings from the brief were verified on both CPU (`designos-render`) and GPU (`designos-gpu`).
- `large_file_compile_benchmarks` in `designos-gpu` was intentionally skipped per the common rules (`--skip large_file_compile_benchmarks`, verified filtered out at `plans/reports/agy-1277-shadow-knockout-evidence/gates.log:1988`).

---

## Step 1 — Facts First (Recorded Before Any Code Edits on `06e6b84a`)

### 1.1 Studio Re-Anchor (`06e6b84a`)

1. **How a fill-less frame's drop shadow is built and knocked out (`show_behind = false`)**:
   - **CPU (`designos-render`)**:
     - `Renderer::draw_body_inner` (`crates/designos-render/src/lib.rs:595-626`) defines the silhouette callback `sil` (`crates/designos-render/src/lib.rs:604-606`), which invokes `self.draw_silhouette(n, ts, shape.as_ref(), text_layout.as_deref(), pm, dx, dy, grow, mode)`.
     - For `Effect::DropShadow { show_behind: false, .. }` (`crates/designos-render/src/lib.rs:607-624`), a fill-less frame returns `None` from `self.solid_rrect(n, shape.as_ref())` (`crates/designos-render/src/lib.rs:555-562`, which requires `fills.len() == 1` with `Paint::Solid`), so it dispatches to `effects::drop_shadow` (`crates/designos-render/src/lib.rs:618` -> `crates/designos-render/src/effects.rs:85-151`).
     - `effects::drop_shadow_with` (`crates/designos-render/src/effects.rs:141-149`) renders the shadow source into `pm` via `sil(&mut pm, -(rx as f32) + ox, -(ry as f32) + oy, spread, SilhouetteMode::Alpha)` (`crates/designos-render/src/effects.rs:141`), blurs and colorizes `pm` (`crates/designos-render/src/effects.rs:142-143`), and then when `!show_behind` (`crates/designos-render/src/effects.rs:144-149`), allocates `knock`, renders the knockout mask via `sil(&mut knock, -(rx as f32), -(ry as f32), 0.0, SilhouetteMode::Knockout)` (`crates/designos-render/src/effects.rs:146`), and composites `knock` onto `pm` with `sk::BlendMode::DestinationOut` (`crates/designos-render/src/effects.rs:147`).
   - **GPU (`designos-gpu`)**:
     - `SceneCompiler::draw_body_inner` (`crates/designos-gpu/src/compile.rs:1028-1128`) dispatches `Effect::DropShadow` to `self.drop_shadow(...)` (`crates/designos-gpu/src/compile.rs:1118` -> `crates/designos-gpu/src/compile.rs:1452-1498`).
     - A fill-less frame returns `false` from `self.drop_shadow_analytic(...)` (`crates/designos-gpu/src/compile.rs:1414-1415`, which requires `fills.len() == 1`), builds the shadow source offscreen program `s` via `c.sil_program(n, g, ts, rect, ox, oy, spread, SilhouetteMode::Alpha)` (`crates/designos-gpu/src/compile.rs:1489-1491`), blurs `s` (`crates/designos-gpu/src/compile.rs:1492-1494`), and when `!show_behind`, builds the knockout program via `self.sil_program(n, g, ts, rect, 0.0, 0.0, 0.0, SilhouetteMode::Knockout)` with `AuxMode::MulInv` (`crates/designos-gpu/src/compile.rs:1495`).
   - **Shared `silhouette_source` decision (`designos-doc`)**:
     - Both `Renderer::draw_silhouette` (`crates/designos-render/src/lib.rs:731`) and `SceneCompiler::silhouette` (`crates/designos-gpu/src/compile.rs:1289`) call `silhouette_source(n, shape.map(|s| s.fillable), mode)` (`crates/designos-doc/src/silhouette.rs:61-85`).
     - For a fill-less frame with children (`shape_fillable = Some(true)`, no visible fills or strokes, `has_children = true`), `SilhouetteMode::Alpha` falls through to `SilhouetteSource::Children` (`crates/designos-doc/src/silhouette.rs:80-81`), calling `draw_children` on CPU (`crates/designos-render/src/lib.rs:788-797`) and GPU (`crates/designos-gpu/src/compile.rs:1335-1341`), whereas `SilhouetteMode::Knockout` hits `if fillable && mode == SilhouetteMode::Knockout && has_children` (`crates/designos-doc/src/silhouette.rs:70-75`) and returns `SilhouetteSource::ChildrenOpaque`, calling `knockout_children` on CPU (`crates/designos-render/src/lib.rs:798-810`) and GPU (`crates/designos-gpu/src/compile.rs:1342-1347`).

2. **Where the knockout walk differs from the child paint walk on `06e6b84a` (the four cases in `#1277`)**:
   - **Case 1 — A child that paints nothing still knocks out its rectangle**:
     - `has_visible_fills(n)` (`crates/designos-doc/src/silhouette.rs:87-89`) checked `n.fills.iter().any(|p| p.visible)` without checking `p.silhouette_alpha() > 0.0` (`Paint::silhouette_alpha` at `crates/designos-doc/src/paint.rs:239-245`), and `has_visible_strokes(n)` (`crates/designos-doc/src/silhouette.rs:91-94`) checked `n.stroke.paints.iter().any(|p| p.visible)` without checking `p.silhouette_alpha() > 0.0`. Thus a child with a 0% opacity fill (`opacity == 0.0` or `Color::TRANSPARENT`) returned `SilhouetteSource::OwnShape` (`crates/designos-doc/src/silhouette.rs:67-69`), and in `Knockout` mode (`opaque = mode.ignores_alpha() = true` at `crates/designos-doc/src/silhouette.rs:20-22`), `draw_silhouette` (`crates/designos-render/src/lib.rs:752-757`) and `SceneCompiler::silhouette` (`crates/designos-gpu/src/compile.rs:1307-1310`) painted its full shape at alpha `1.0` instead of walking its children or skipping it.
     - Furthermore, when a fill-less, stroke-less fillable child had no children (`has_children == false`, such as an empty child frame or a child frame with a hidden fill), `if fillable && mode == SilhouetteMode::Knockout && has_children` (`crates/designos-doc/src/silhouette.rs:70`) was `false`, so execution fell through to `if fillable && mode.ignores_alpha()` (`crates/designos-doc/src/silhouette.rs:76-78`) and returned `SilhouetteSource::OwnShape`. Both `draw_silhouette` (`crates/designos-render/src/lib.rs:747-750`) and `SceneCompiler::silhouette` (`crates/designos-gpu/src/compile.rs:1304-1305`) then hit `if sh.fillable && opaque && fills.is_empty()` and filled the child's entire rectangle with `black_paint(1.0)`.
   - **Case 2 — Masks are skipped in `knockout_children`**:
     - On CPU, `Renderer::knockout_children` (`crates/designos-render/src/lib.rs:692-711`) checked `if !child.visible || child.is_mask || matches!(child.kind, NodeKind::Page { .. }) || child.opacity * runtime_mul <= 0.0 { continue; }` (`crates/designos-render/src/lib.rs:700`), skipping any `child.is_mask` and knocking out all subsequent siblings unmasked. By contrast, `Renderer::draw_children` (`crates/designos-render/src/lib.rs:391-417`) intercepts `child.is_mask`, renders `&children[i + 1..]` into an offscreen `layer`, renders the mask stencil (`mask::mask_stencil(child)`, `MaskType::Vector`, `MaskType::Luminance`) into `mask_layer`, composites `mask_layer` onto `layer` with `sk::BlendMode::DestinationIn`, and composites `layer` onto `target`.
     - On GPU, `SceneCompiler::knockout_children` (`crates/designos-gpu/src/compile.rs:1376-1389`) likewise skipped `child.is_mask` (`crates/designos-gpu/src/compile.rs:1382`), whereas `SceneCompiler::draw_children` (`crates/designos-gpu/src/compile.rs:732-738`) calls `self.draw_mask(child, children, i, parent_ts, ts, clip)` (`crates/designos-gpu/src/compile.rs:763-822`).
   - **Case 3 — A group keeps child alpha in `Knockout` mode**:
     - For `NodeKind::Group` (where `shape_fillable` is `None`), `silhouette_source` (`crates/designos-doc/src/silhouette.rs:80-84`) fell past `if let Some(fillable) = shape_fillable` and returned `SilhouetteSource::Children` even when `mode == SilhouetteMode::Knockout`.
     - In `draw_silhouette` (`crates/designos-render/src/lib.rs:788-797`) and `SceneCompiler::silhouette` (`crates/designos-gpu/src/compile.rs:1335-1341`), `SilhouetteSource::Children` called `draw_children` instead of `knockout_children`, so a 50% opacity child inside a `Group` wrote alpha `0.5` into the knockout mask instead of `1.0`.
   - **Case 4 — GPU knockout ignores runtime opacity 0**:
     - CPU `Renderer::knockout_children` (`crates/designos-render/src/lib.rs:699-700`) multiplied `child.opacity` by `runtime_mul` from `self.runtime_opacities` and skipped the child when `child.opacity * runtime_mul <= 0.0`.
     - GPU `SceneCompiler::knockout_children` (`crates/designos-gpu/src/compile.rs:1382`) checked only `child.opacity <= 0.0` without consulting `self.opts.runtime_node_opacities` (unlike `SceneCompiler::draw_node` at `crates/designos-gpu/src/compile.rs:931-933`).

### 1.2 Figma Facts via REA (`compiled_wasm.wasm` & `figma_app.min.js`)

- **Artifacts inspected with `rea`**:
  - `/Applications/Figma.app/Contents/MacOS/Figma` via `rea inspect-macho` -> evidence bundle `ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:2`).
  - Figma Desktop profile HTTP cache (`~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data`) decompressed and analyzed via `rea analyze-javascript-application` -> evidence bundle `ev_9fce9b351dc7d7ed6e08f0908ddbd9c080300282ceee03315d8003dda1697fad` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:3`):
    - `d48a6add9e1dc400_0` -> `compiled_wasm.wasm` (`53,282,791` bytes, `sha256=288ca99bafef25576d63d33f88a426fa328ce18b504f43c7efe71a79ce11a60b`) (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:6`).
    - `7d063ae82a1f1003_0` -> `compiled_wasm.js` (`2,377,189` bytes, `sha256=4b5b963d4d7701eabfe283997bbe6af7261041b82ea5b73821d1b769b702cb1f`) (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:7`).
    - `613e12ea20f89370_0` -> `figma_app.min.js` (`8,369,519` bytes, `sha256=441fee61932dfa797060c18d5f764bbe0e26e62d403c798ec2ddc3b4bd412b07`) (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:8`).
- **How Figma builds and composites a fill-less frame's drop-shadow knockout**:
  1. **Binary knockout shader (`MaskKnockoutShadowTile` / `useBinaryKnockoutMask`)**:
     - In `compiled_wasm.wasm` at file offset `50567144`, `MaskOpType` defines `NORMAL=0, LUMINANCE=1, INNER_SHADOW=2, BACKGROUND_BLUR=3, KNOCKOUT_SHADOW=4, GLASS=5` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:12`), printed as ` type=KNOCKOUT_SHADOW` at string offset `49694319` (`vaddr=1316338`) by `func[3983]` at code offset `@1864127` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:13,31`).
     - In `MaskKnockoutShadowTile` at file offset `48415780..48416300` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:18,21`), the fragment shader samples the knockout mask alpha `float a = texture2D(texture2, _coord2).a; float b = float(a == 0.);` and when `useBinaryKnockoutMask != 0` (`knockout_shadow_binary_mask` feature flag at file offset `49440988`, checked in `func[6120]` at `@3494373..3495100` where `MaskOpType = 4` is stored at offset `52`) outputs `gl_FragColor = mix(c, d, b);` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:14,21,135-143`). Any pixel with `a > 0.0` in the knockout mask has `b = 0.0` (100% shadow removal), and any pixel with `a == 0.0` has `b = 1.0` (shadow preserved).
  2. **RenderTree construction for filled vs. fill-less frames (`func[8653]` & `func[21535]`)**:
     - In `func[21535]` (`@20902139..20902550`, called from the node RenderTree builder `func[8653]`), `@20902216` calls `func[4274]` (`@2178564..2178754`) to build the node's own fill/stroke geometry render node into `local 8` (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:44-131,193`).
     - If `local 8 != 0` (`@20902265`), the node's own fill/stroke geometry is used as the knockout mask (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:198-199`).
     - If `local 8 == 0` (the frame has no visible fill/stroke geometry of its own) and `fullscreen_knockout_shadow_optimizations` (`func[13544]` at `@11639894..11639927`, string at file offset `49241854`, called at `@20902277`) is enabled, `@20902305..20902474` clones the vector of the children's already-constructed RenderTree nodes (built by the child loop in `func[8653]` at `@5993502..5993841`) and wraps them in a `GROUP` render node via `func[2182]` (`@20902474`) before passing that group to `func[6120]` (`KNOCKOUT_SHADOW`) (`plans/reports/agy-1277-shadow-knockout-evidence/step1-figma-re.log:15,148-225`).

---

## Step 2 — Code Changes (`file:line`)

1. **`crates/designos-doc/src/silhouette.rs` (`silhouette_source`, `has_visible_fills`, `has_visible_strokes`)**:
   - `has_visible_fills` (`crates/designos-doc/src/silhouette.rs:89-91`) now requires `p.visible && p.silhouette_alpha() > 0.0`, and `has_visible_strokes` (`crates/designos-doc/src/silhouette.rs:94-96`) now requires `p.visible && p.silhouette_alpha() > 0.0`, so 0%-opacity and `Color::TRANSPARENT` paints do not trigger `SilhouetteSource::OwnShape`.
   - `silhouette_source` (`crates/designos-doc/src/silhouette.rs:61-87`) now checks `if fillable && mode == SilhouetteMode::Opaque` (instead of `mode.ignores_alpha()`) for fill-less leaf shapes, and for any non-painting container with `has_children == true` (including fill-less `Frame`s and `Group`s where `shape_fillable == None`) returns `SilhouetteSource::ChildrenOpaque` (via `FILLLESS_KNOCKOUT`) when `mode == SilhouetteMode::Knockout`, and `SilhouetteSource::Nothing` when `!has_children`.
2. **`crates/designos-render/src/lib.rs` (`Renderer::knockout_children`)**:
   - `Renderer::knockout_children` (`crates/designos-render/src/lib.rs:692-736`) now handles `if child.is_mask` (`crates/designos-render/src/lib.rs:703-723`) identically to `Renderer::draw_children` (`crates/designos-render/src/lib.rs:391-417`): it renders `&children[i + 1..]` with `self.knockout_children` into `layer`, renders the mask stencil (`mask::mask_stencil(child)`, supporting `MaskType::Vector` on `NodeKind::Group` and `MaskType::Luminance` via `mask::apply_luminance_mask`) into `mask_layer` with culling disabled, applies `sk::BlendMode::DestinationIn`, composites `layer` onto `pm`, and returns.
3. **`crates/designos-gpu/src/compile.rs` (`SceneCompiler::knockout_children` & `SceneCompiler::knockout_mask`)**:
   - `SceneCompiler::knockout_children` (`crates/designos-gpu/src/compile.rs:1376-1396`) now intercepts `if child.is_mask` (`crates/designos-gpu/src/compile.rs:1386-1389`) and dispatches to `self.knockout_mask(child, children, i, parent_ts, ts, clip)` (`crates/designos-gpu/src/compile.rs:1398-1447`), which mirrors `SceneCompiler::draw_mask` (`crates/designos-gpu/src/compile.rs:763-822`) while keeping the interactive program-cap/budget guard behind `self.prog_cap_hit(64) || self.over_budget(...)` (`crates/designos-gpu/src/compile.rs:1410-1414`).
   - `SceneCompiler::knockout_children` (`crates/designos-gpu/src/compile.rs:1390-1393`) now multiplies `child.opacity` by `runtime_mul` from `self.opts.runtime_node_opacities` and skips the child when `child.opacity * runtime_mul <= 0.0`.

---

## Step 3 — Acceptance Verification

| Check | Status | Evidence Citation |
|---|---|---|
| **1(a)** Fill-less frame with paint-free child frame & 0%-fill child frame keeps shadow (`RED` on `06e6b84a` -> `GREEN`) | PASS | RED: `plans/reports/agy-1277-shadow-knockout-evidence/red-tests.log:77-78,145-146`; GREEN: `plans/reports/agy-1277-shadow-knockout-evidence/green-tests.log:60,81` (`crates/designos-render/tests/shadow_fillless.rs:233-243`, `crates/designos-gpu/tests/shadow_fillless_fixture.rs:136-156`) |
| **1(b)** Fill-less child frame with painting grandchild knocks out only grandchild's area (`RED` on `06e6b84a` -> `GREEN`) | PASS | RED: `plans/reports/agy-1277-shadow-knockout-evidence/red-tests.log:70-71,156-157`; GREEN: `plans/reports/agy-1277-shadow-knockout-evidence/green-tests.log:63,82` (`crates/designos-render/tests/shadow_fillless.rs:274-286`, `crates/designos-gpu/tests/shadow_fillless_fixture.rs:186-205`) |
| **1(c)** Ellipse mask + rectangle sibling knocks out circle, not rectangle (`RED` on `06e6b84a` -> `GREEN`) | PASS | RED: `plans/reports/agy-1277-shadow-knockout-evidence/red-tests.log:74-75,166-167`; GREEN: `plans/reports/agy-1277-shadow-knockout-evidence/green-tests.log:65,83` (`crates/designos-render/tests/shadow_fillless.rs:321-336`, `crates/designos-gpu/tests/shadow_fillless_fixture.rs:241-261`) |
| **1(d)** 50% child wrapped in group removes same shadow as direct child (`RED` on `06e6b84a` -> `GREEN`) | PASS | RED: `plans/reports/agy-1277-shadow-knockout-evidence/red-tests.log:83-86,176-179`; GREEN: `plans/reports/agy-1277-shadow-knockout-evidence/green-tests.log:64,84` (`crates/designos-render/tests/shadow_fillless.rs:362-371`, `crates/designos-gpu/tests/shadow_fillless_fixture.rs:287-305`) |
| **1(e)** GPU skips runtime-opacity-0 child like CPU (`RED` on `06e6b84a` -> `GREEN`) | PASS | RED: `plans/reports/agy-1277-shadow-knockout-evidence/red-tests.log:184-185`; GREEN: `plans/reports/agy-1277-shadow-knockout-evidence/green-tests.log:59,85` (`crates/designos-render/tests/shadow_fillless.rs:374-396`, `crates/designos-gpu/tests/shadow_fillless_fixture.rs:308-339`) |
| **1(null)** Null control: `#520` fixtures, filled frame, `show_behind: true`, and plain fill-less frame render identical pixels (`0x92c8d1f05c89c3be`, `0x67f982db879975e7`, `0x31ff0b365a24b628`) | PASS | Base `06e6b84a`: `plans/reports/agy-1277-shadow-knockout-evidence/red-tests.log:69-70,98,120-121,134`; Post-fix: `plans/reports/agy-1277-shadow-knockout-evidence/green-tests.log:54-58,66-67,79-80,86,98-102` (`crates/designos-render/tests/shadow_fillless.rs:407-420`, `crates/designos-gpu/tests/shadow_fillless_fixture.rs:342-351`) |
| **2(M1)** Mutant 1 killed (paint-free children knock out their rectangle again) with green null control | PASS | `plans/reports/agy-1277-shadow-knockout-evidence/mutants.log:1-66` (kills `knockout_ignores_zero_alpha_fills_and_strokes`, `knockout_of_fill_less_frame_is_its_children_not_its_rectangle`, `issue_1277_a_*`, and `issue_1277_b_*` on CPU and GPU; null control `ok` at `mutants.log:41,60`) |
| **2(M2)** Mutant 2 killed (mask ignored in knockout walk) with green null control | PASS | `plans/reports/agy-1277-shadow-knockout-evidence/mutants.log:68-106` (kills `issue_1277_c_*` on CPU and GPU; null control `ok` at `mutants.log:85,102`) |
| **3** CPU and GPU crops of `(a)`, `(c)`, and `(d)`, before and after | PASS | `plans/reports/agy-1277-shadow-knockout-evidence/case-a-cpu-before.png`, `case-a-gpu-before.png`, `case-a-cpu-after.png`, `case-a-gpu-after.png`, `case-c-cpu-before.png`, `case-c-gpu-before.png`, `case-c-cpu-after.png`, `case-c-gpu-after.png`, `case-d-cpu-before.png`, `case-d-gpu-before.png`, `case-d-cpu-after.png`, `case-d-gpu-after.png` |
| **4.1** `cargo fmt --all -- --check` | PASS | `fmt_rc=0` (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:1-2`) |
| **4.2** `cargo clippy` (`-p designos-doc -p designos-render -p designos-gpu --all-targets --locked -- -D warnings` + `--workspace --all-targets --locked`) | PASS | `clippy_touched_rc=0` (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:3,226`); workspace clippy has only `designos-app`'s known warnings (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:227-639`) |
| **4.3** `cargo test -p designos-doc --locked` | PASS | `724 passed; 0 failed` (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:1374-1408`) |
| **4.4** `cargo test -p designos-render --locked` | PASS | All 30 test binaries passed, `0 failed` (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:1410-1899`) |
| **4.5** `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | PASS | All 16 test binaries passed, `0 failed`, `large_file_compile_benchmarks` `1 filtered out` (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:1901-2118`, `1988`) |
| **4.6** `cargo test --workspace --locked --lib` | PASS | All 12 workspace library test suites passed, `0 failed` (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:2120-3844`) |
| **4.7** `python3 -B scripts/zero_fake_ui_gate.py` | PASS | `PASS (17/17 invariants satisfied)`, `zero_fake_rc=0` (`plans/reports/agy-1277-shadow-knockout-evidence/gates.log:3846-3872`) |

### Pasted Test & Gate Excerpts

#### RED Run on Base `06e6b84a` (`plans/reports/agy-1277-shadow-knockout-evidence/red-tests.log:1-197`)

```text
=== base commit ===
06e6b84af2e1a70200ea123156b702a76d0813df
=== 1. cargo test -p designos-doc --locked silhouette ===
test silhouette::tests::knockout_of_group_and_text_is_children_opaque_and_text ... FAILED
test silhouette::tests::knockout_ignores_zero_alpha_fills_and_strokes ... FAILED
test silhouette::tests::knockout_of_fill_less_frame_is_its_children_not_its_rectangle ... FAILED
test result: FAILED. 12 passed; 3 failed; 0 ignored; 0 measured; 709 filtered out; finished in 0.01s

=== 2. cargo test -p designos-render --locked --test shadow_fillless ===
thread 'issue_1277_b_fillless_child_frame_with_painting_grandchild_knocks_out_only_grandchild' panicked at crates/designos-render/tests/shadow_fillless.rs:279:5:
fill-less child frame knocked out shadow outside grandchild: [255, 255, 255, 255]
thread 'issue_1277_c_ellipse_mask_plus_rect_sibling_knocks_out_circle_not_rect' panicked at crates/designos-render/tests/shadow_fillless.rs:326:5:
unmasked rect corner knocked out circular shadow at (32, 32): [255, 255, 255, 255]
thread 'issue_1277_a_paint_free_and_zero_fill_children_keep_shadow' panicked at crates/designos-render/tests/shadow_fillless.rs:238:5:
shadow under paint-free child frame was knocked out: [255, 255, 255, 255]
thread 'issue_1277_d_group_wrapped_50pct_child_removes_same_shadow_as_direct_child' panicked at crates/designos-render/tests/shadow_fillless.rs:366:5:
assertion `left == right` failed: group wrapper changed 6000 shadow pixels
test issue_1277_null_control_filled_and_plain_fillless_frames_unchanged ... ok
test result: FAILED. 10 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

=== 3. cargo test -p designos-gpu --locked --test shadow_fillless_fixture (SAVE_1277_CROPS=before) ===
test cpu_and_gpu_match_on_the_fixture_pair ... ok
test cpu_and_gpu_match_on_the_fixture_pair_with_show_behind ... ok
test issue_1277_a_paint_free_and_zero_fill_children_keep_shadow ... FAILED
test issue_1277_b_fillless_child_frame_with_painting_grandchild_knocks_out_only_grandchild ... FAILED
test issue_1277_c_ellipse_mask_plus_rect_sibling_knocks_out_circle_not_rect ... FAILED
test issue_1277_d_group_wrapped_50pct_child_removes_same_shadow_as_direct_child ... FAILED
test issue_1277_e_gpu_skips_runtime_opacity_zero_child_like_cpu ... FAILED
test issue_1277_null_control_filled_and_plain_fillless_frames_unchanged ... ok
test result: FAILED. 3 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.47s
```

#### GREEN Run After Fix (`plans/reports/agy-1277-shadow-knockout-evidence/green-tests.log:1-105`)

```text
=== 1. cargo test -p designos-doc --locked silhouette ===
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 709 filtered out; finished in 0.03s

=== 2. cargo test -p designos-render --locked --test shadow_fillless ===
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

=== 3. cargo test -p designos-gpu --locked --test shadow_fillless_fixture (SAVE_1277_CROPS=after) ===
test cpu_and_gpu_match_on_the_fixture_pair ... ok
test cpu_and_gpu_match_on_the_fixture_pair_with_show_behind ... ok
test issue_1277_a_paint_free_and_zero_fill_children_keep_shadow ... ok
test issue_1277_b_fillless_child_frame_with_painting_grandchild_knocks_out_only_grandchild ... ok
test issue_1277_c_ellipse_mask_plus_rect_sibling_knocks_out_circle_not_rect ... ok
test issue_1277_d_group_wrapped_50pct_child_removes_same_shadow_as_direct_child ... ok
test issue_1277_e_gpu_skips_runtime_opacity_zero_child_like_cpu ... ok
test issue_1277_null_control_filled_and_plain_fillless_frames_unchanged ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.22s

=== 4. cargo test -p designos-gpu --locked --test shadow_silhouette_source ===
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.79s
```

#### Mutant Kills (`plans/reports/agy-1277-shadow-knockout-evidence/mutants.log:1-106`)

```text
=== MUTANT 1: paint-free children knock out their rectangle again ===
$ cargo test -p designos-doc --locked silhouette -> exit_code=101 (13 passed; 2 failed)
$ cargo test -p designos-render --locked --test shadow_fillless -> exit_code=101 (12 passed; 2 failed: issue_1277_a_*, issue_1277_b_*; null_control ok)
$ cargo test -p designos-gpu --locked --test shadow_fillless_fixture -- --test-threads=1 -> exit_code=101 (6 passed; 2 failed: issue_1277_a_*, issue_1277_b_*; null_control ok)

=== MUTANT 2: mask is ignored in the knockout walk ===
$ cargo test -p designos-render --locked --test shadow_fillless -> exit_code=101 (13 passed; 1 failed: issue_1277_c_*; null_control ok)
$ cargo test -p designos-gpu --locked --test shadow_fillless_fixture -- --test-threads=1 -> exit_code=101 (7 passed; 1 failed: issue_1277_c_*; null_control ok)
```

---

## Remaining Assumptions & Weakest Link

- **Remaining assumptions**: None on the five rulings; `Paint::silhouette_alpha` (`crates/designos-doc/src/paint.rs:239-245`) already returns `0.0` for `PaintKind::Solid(c)` with `c.a == 0.0`, gradients whose stops all have `color.a == 0.0`, and any paint with `opacity == 0.0`.
- **Weakest link**: On CPU (`crates/designos-render/src/lib.rs:703-723`), applying a sibling mask inside `knockout_children` allocates two temporary `Pixmap`s (`layer` and `mask_layer`) of the shadow pixmap's dimensions just like `Renderer::draw_children` (`crates/designos-render/src/lib.rs:396-399`) does. Since the shadow pixmap is already bounded to the node's shadow bbox (`effects::drop_shadow_with` at `crates/designos-render/src/effects.rs:133-145`), this matches the existing mask allocation cost in the shadow source pass (`SilhouetteMode::Alpha`).

---

## Step 4 — Draft PR, Git Log & Cleanup

- **Draft PR URL**: https://github.com/jangtrinh/design-os-studio/pull/1569
- **`git log --oneline origin/main..HEAD`**:

```text
534cc2d1 fix(effects): fill-less frame shadow knockout follows what the children paint (#1277)
```

- **`ls out/cargo` after `rm -rf out/cargo`**:

```text
$ ls out/cargo
ls: out/cargo: No such file or directory
```

Status: DONE
